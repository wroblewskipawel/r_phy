use std::{
    cell::RefCell,
    error::Error,
    ffi::c_void,
    fmt::{self, Display, Formatter},
    rc::Rc,
};

use ash::{vk, Device};

use super::{buffer::ByteRange, PhysicalDeviceProperties, VulkanDevice};

#[derive(Debug, Clone, Copy)]
pub enum DeviceAllocError {
    OutOfMemory,
    UnsupportedMemoryType,
    VulkanError(vk::Result),
}

impl Display for DeviceAllocError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            DeviceAllocError::OutOfMemory => write!(f, "Out of memory"),
            DeviceAllocError::UnsupportedMemoryType => write!(f, "Unsupported memory type"),
            DeviceAllocError::VulkanError(err) => write!(f, "Vulkan error: {}", err),
        }
    }
}

impl From<vk::Result> for DeviceAllocError {
    fn from(err: vk::Result) -> Self {
        DeviceAllocError::VulkanError(err)
    }
}

impl Error for DeviceAllocError {}

#[derive(Debug)]
pub struct MemoryBlock {
    // TODO: Temporary pub for compatibility with oterh modules, to be removed
    pub memory: vk::DeviceMemory,
    range: ByteRange,
    page: Rc<RefCell<MemoryPage>>,
    ptr: Option<*mut c_void>,
}

impl MemoryBlock {
    pub fn bind_buffer_memory(
        &self,
        device: &Device,
        buffer: vk::Buffer,
    ) -> Result<(), vk::Result> {
        unsafe { device.bind_buffer_memory(buffer, self.memory, self.range.beg as vk::DeviceSize) }
    }

    pub fn bind_image_memory(&self, device: &Device, image: vk::Image) -> Result<(), vk::Result> {
        unsafe { device.bind_image_memory(image, self.memory, self.range.beg as vk::DeviceSize) }
    }

    pub fn map_memory(
        &mut self,
        device: &Device,
        range: ByteRange,
    ) -> Result<*mut c_void, vk::Result> {
        if self.ptr.is_none() {
            self.ptr = Some(self.page.borrow_mut().map_page(device)?);
        }
        Ok(unsafe { self.ptr.unwrap().byte_add(self.range.beg + range.beg) })
    }

    pub fn unmap_memory(&mut self, device: &Device) {
        if self.ptr.is_some() {
            self.page.borrow_mut().unmap_page(device);
            self.ptr = None;
        }
    }
}

#[derive(Debug)]
pub struct MemoryPage {
    memory: vk::DeviceMemory,
    alloc_size: vk::DeviceSize,
    alloc_range: ByteRange,
    ptr: Option<*mut c_void>,
    mapped_chunks: usize,
}

impl MemoryPage {
    pub fn try_allocate(
        cell: &Rc<RefCell<Self>>,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<MemoryBlock> {
        let mut page = cell.borrow_mut();
        if let Some(range) = page
            .alloc_range
            .alloc_raw(size as usize, alignment as usize)
        {
            Some(MemoryBlock {
                memory: page.memory,
                range,
                page: cell.clone(),
                ptr: None,
            })
        } else {
            None
        }
    }

    pub fn map_page(&mut self, device: &Device) -> Result<*mut c_void, vk::Result> {
        if self.ptr.is_none() {
            self.ptr = Some(unsafe {
                device.map_memory(self.memory, 0, self.alloc_size, vk::MemoryMapFlags::empty())?
            });
        };
        self.mapped_chunks.checked_add(1).unwrap();
        Ok(self.ptr.unwrap())
    }

    pub fn unmap_page(&mut self, device: &Device) {
        if let Some(mapped_chunks) = self.mapped_chunks.checked_sub(1) {
            self.mapped_chunks = mapped_chunks;
            if self.mapped_chunks == 0 {
                unsafe {
                    device.unmap_memory(self.memory);
                }
                self.ptr = None
            }
        }
    }
}

struct MemoryType {
    index: u32,
    page_size: vk::DeviceSize,
    pages: Vec<Rc<RefCell<MemoryPage>>>,
}

impl MemoryType {
    pub fn try_allocate(
        &mut self,
        device: &Device,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<MemoryBlock> {
        self.pages
            .iter()
            .find_map(|page| MemoryPage::try_allocate(page, size, alignment))
            .or_else(|| {
                self.allocate_page(device, (size / self.page_size + 1) * self.page_size)
                    .ok()
                    .and_then(|page| MemoryPage::try_allocate(&page, size, alignment))
            })
    }

    pub fn allocate_page(
        &mut self,
        device: &Device,
        page_size: vk::DeviceSize,
    ) -> Result<Rc<RefCell<MemoryPage>>, DeviceAllocError> {
        self.pages.push(Rc::new(RefCell::new(MemoryPage {
            memory: unsafe {
                device.allocate_memory(
                    &vk::MemoryAllocateInfo {
                        allocation_size: page_size,
                        memory_type_index: self.index,
                        ..Default::default()
                    },
                    None,
                )?
            },
            alloc_size: page_size,
            alloc_range: ByteRange::new(page_size as usize),
            ptr: None,
            mapped_chunks: 0,
        })));
        Ok(self.pages.last().unwrap().clone())
    }
}

pub struct MemoryAllocator {
    memory_types: Vec<MemoryType>,
}

pub struct AllocReq {
    properties: vk::MemoryPropertyFlags,
    requirements: vk::MemoryRequirements,
}

impl AllocReq {
    pub fn new(properties: vk::MemoryPropertyFlags, requirements: vk::MemoryRequirements) -> Self {
        AllocReq {
            properties,
            requirements,
        }
    }
}

impl MemoryAllocator {
    pub fn create(
        properties: &PhysicalDeviceProperties,
        page_size: vk::DeviceSize,
    ) -> Result<MemoryAllocator, Box<dyn Error>> {
        let memory_types = (0..properties.memory.memory_types.len() as u32)
            .map(|index| MemoryType {
                page_size,
                index: index as u32,
                pages: Vec::new(),
            })
            .collect();
        Ok(MemoryAllocator { memory_types })
    }

    pub fn destroy(&mut self, device: &Device) {
        self.memory_types.drain(0..).for_each(|mut memory_type| {
            memory_type.pages.drain(0..).for_each(|page| unsafe {
                device.free_memory(page.borrow_mut().memory, None);
            })
        });
    }
}

impl VulkanDevice {
    pub fn allocate_memory(&mut self, request: AllocReq) -> Result<MemoryBlock, DeviceAllocError> {
        let memory_type_index = self
            .get_memory_type_index(request.requirements.memory_type_bits, request.properties)
            .ok_or(DeviceAllocError::UnsupportedMemoryType)?;
        self.memory_allocator.memory_types[memory_type_index as usize]
            .try_allocate(
                &self.device,
                request.requirements.size,
                request.requirements.alignment,
            )
            .ok_or(DeviceAllocError::OutOfMemory)
    }

    pub fn get_memory_type_index(
        &self,
        memory_type_bits: u32,
        memory_properties: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        self.physical_device
            .properties
            .memory
            .memory_types
            .iter()
            .zip(0u32..)
            .find_map(|(memory, type_index)| {
                if (1 << type_index & memory_type_bits == 1 << type_index)
                    && memory.property_flags.contains(memory_properties)
                {
                    Some(type_index)
                } else {
                    None
                }
            })
    }
}
