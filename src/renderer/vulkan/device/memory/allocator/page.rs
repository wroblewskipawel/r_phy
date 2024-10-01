use std::{
    cell::RefCell,
    error::Error,
    ffi::c_void,
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    rc::Rc,
};

use ash::{vk, Device};

use crate::renderer::vulkan::{
    device::{
        memory::{
            HostCoherent, HostVisibleMemory, Memory, MemoryChunk, MemoryChunkRaw, MemoryProperties,
        },
        resources::buffer::ByteRange,
        VulkanDevice,
    },
    VulkanRendererConfig,
};

use super::{AllocReqTyped, Allocator, AllocatorCreate, DeviceAllocError};

pub struct PageChunk<M: MemoryProperties> {
    chunk: MemoryChunk<M>,
    page: Rc<RefCell<Page>>,
    ptr: Option<*mut c_void>,
}

impl<M: MemoryProperties> Debug for PageChunk<M> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("MemoryBlock")
            .field("chunk", &self.chunk)
            .field("page", &self.page)
            .field("ptr", &self.ptr)
            .finish()
    }
}

impl<M: MemoryProperties> Memory for PageChunk<M> {
    type Properties = M;
    fn chunk(&self) -> MemoryChunk<Self::Properties> {
        self.chunk
    }
}

impl HostVisibleMemory for PageChunk<HostCoherent> {
    fn map_memory(&mut self, device: &Device, range: ByteRange) -> Result<*mut c_void, vk::Result> {
        if self.ptr.is_none() {
            self.ptr = Some(self.page.borrow_mut().map_page(device)?);
        }
        Ok(unsafe { self.ptr.unwrap().byte_add(self.chunk.range.beg + range.beg) })
    }

    fn unmap_memory(&mut self, device: &Device) {
        if self.ptr.is_some() {
            self.page.borrow_mut().unmap_page(device);
            self.ptr = None;
        }
    }
}

#[derive(Debug)]
pub struct Page {
    memory: vk::DeviceMemory,
    alloc_size: vk::DeviceSize,
    alloc_range: ByteRange,
    ptr: Option<*mut c_void>,
    mapped_chunks: usize,
}

impl Page {
    pub fn try_allocate<M: MemoryProperties>(
        cell: &Rc<RefCell<Self>>,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<PageChunk<M>> {
        let mut page = cell.borrow_mut();
        if let Some(range) = page
            .alloc_range
            .alloc_raw(size as usize, alignment as usize)
        {
            Some(PageChunk {
                chunk: MemoryChunk {
                    raw: MemoryChunkRaw {
                        memory: page.memory,
                        range,
                    },
                    _phantom: PhantomData,
                },
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

#[derive(Debug)]
struct PageType {
    index: u32,
    pages: Vec<Rc<RefCell<Page>>>,
}

impl PageType {
    pub fn allocate_page(
        &mut self,
        device: &Device,
        page_size: vk::DeviceSize,
    ) -> Result<Rc<RefCell<Page>>, DeviceAllocError> {
        self.pages.push(Rc::new(RefCell::new(Page {
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

#[derive(Debug, Clone, Copy)]
pub struct PageAllocatorConfig {
    page_size: vk::DeviceSize,
}

impl<'a> From<&'a VulkanRendererConfig> for PageAllocatorConfig {
    fn from(value: &'a VulkanRendererConfig) -> Self {
        Self {
            page_size: value.page_size,
        }
    }
}

#[derive(Debug)]
pub struct PageAllocator {
    memory_types: Vec<PageType>,
    config: PageAllocatorConfig,
}

impl AllocatorCreate for PageAllocator {
    type Config = PageAllocatorConfig;

    fn create(device: &VulkanDevice, config: &Self::Config) -> Result<Self, Box<dyn Error>> {
        let properties = &device.physical_device.properties;
        let memory_types = (0..properties.memory.memory_types.len() as u32)
            .map(|index| PageType {
                index: index as u32,
                pages: Vec::new(),
            })
            .collect();
        Ok(PageAllocator {
            memory_types,
            config: *config,
        })
    }

    fn destroy(&mut self, device: &VulkanDevice) {
        self.memory_types.drain(0..).for_each(|mut memory_type| {
            memory_type.pages.drain(0..).for_each(|page| unsafe {
                device.free_memory(page.borrow_mut().memory, None);
            })
        });
    }
}

impl Allocator for PageAllocator {
    type Allocation<M: MemoryProperties> = PageChunk<M>;

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &VulkanDevice,
        request: AllocReqTyped<M>,
    ) -> Result<Self::Allocation<M>, DeviceAllocError> {
        let memory_type_index = request
            .get_memory_type_index(&device.physical_device.properties.memory)
            .ok_or(DeviceAllocError::UnsupportedMemoryType)?;
        let vk::MemoryRequirements {
            size, alignment, ..
        } = request.requirements;
        let page_type = &mut self.memory_types[memory_type_index as usize];
        page_type
            .pages
            .iter()
            .find_map(|page| Page::try_allocate(page, size, alignment))
            .or_else(|| {
                let rquired_page_size = (size / self.config.page_size + 1) * self.config.page_size;
                page_type
                    .allocate_page(device, rquired_page_size)
                    .ok()
                    .and_then(|page| Page::try_allocate(&page, size, alignment))
            })
            .ok_or(DeviceAllocError::OutOfMemory)
    }

    fn free<M: MemoryProperties>(
        &mut self,
        _device: &VulkanDevice,
        _allocation: &mut Self::Allocation<M>,
    ) {
    }
}
