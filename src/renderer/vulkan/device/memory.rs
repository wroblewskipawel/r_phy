use std::{
    cell::RefCell,
    error::Error,
    ffi::c_void,
    fmt::{self, Debug, Display, Formatter},
    marker::PhantomData,
    rc::Rc,
};

use ash::{vk, Device};

use crate::{core::Nil, renderer::vulkan::VulkanRendererConfig};

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

pub trait MemoryProperties: 'static {
    fn properties() -> vk::MemoryPropertyFlags;
}

#[derive(Debug)]
pub struct HostVisible;

impl MemoryProperties for HostVisible {
    fn properties() -> vk::MemoryPropertyFlags {
        vk::MemoryPropertyFlags::HOST_VISIBLE
    }
}

#[derive(Debug)]
pub struct HostCoherent;

impl MemoryProperties for HostCoherent {
    fn properties() -> vk::MemoryPropertyFlags {
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
    }
}

#[derive(Debug)]
pub struct DeviceLocal;

impl MemoryProperties for DeviceLocal {
    fn properties() -> vk::MemoryPropertyFlags {
        vk::MemoryPropertyFlags::DEVICE_LOCAL
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Resource {
    Buffer(vk::Buffer),
    Image(vk::Image),
}

impl From<vk::Image> for Resource {
    fn from(image: vk::Image) -> Self {
        Resource::Image(image)
    }
}

impl From<vk::Buffer> for Resource {
    fn from(buffer: vk::Buffer) -> Self {
        Resource::Buffer(buffer)
    }
}

#[derive(Debug)]
pub struct AllocReq<T: MemoryProperties> {
    requirements: vk::MemoryRequirements,
    _phantom: PhantomData<T>,
}

impl<T: MemoryProperties> Clone for AllocReq<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: MemoryProperties> Copy for AllocReq<T> {}

impl<M: MemoryProperties> AllocReq<M> {
    pub fn get_memory_type_index(&self, properties: &PhysicalDeviceProperties) -> Option<u32> {
        let memory_type_bits = self.requirements.memory_type_bits;
        let memory_properties = M::properties();

        properties
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

impl VulkanDevice {
    pub fn bind_memory<T: Into<Resource>, M: MemoryProperties, C: Memory<M>>(
        &self,
        resource: T,
        memory: &C,
    ) -> Result<(), vk::Result> {
        let MemoryChunk { memory, range, .. } = memory.chunk();

        match resource.into() {
            Resource::Buffer(buffer) => unsafe {
                self.bind_buffer_memory(buffer, memory, range.beg as vk::DeviceSize)
            },
            Resource::Image(image) => unsafe {
                self.bind_image_memory(image, memory, range.beg as vk::DeviceSize)
            },
        }
    }

    pub fn get_alloc_req<T: Into<Resource>, M: MemoryProperties>(
        &self,
        resource: T,
    ) -> AllocReq<M> {
        let requirements = match resource.into() {
            Resource::Buffer(buffer) => unsafe { self.get_buffer_memory_requirements(buffer) },
            Resource::Image(image) => unsafe { self.get_image_memory_requirements(image) },
        };
        AllocReq {
            requirements,
            _phantom: PhantomData,
        }
    }
}

pub struct MemoryChunk<M: MemoryProperties> {
    memory: vk::DeviceMemory,
    range: ByteRange,
    _phantom: PhantomData<M>,
}

impl<M: MemoryProperties> Debug for MemoryChunk<M> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("MemoryChunk")
            .field("memory", &self.memory)
            .field("range", &self.range)
            .finish()
    }
}

impl<M: MemoryProperties> Clone for MemoryChunk<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: MemoryProperties> Copy for MemoryChunk<M> {}

impl<M: MemoryProperties> MemoryChunk<M> {
    pub fn empty() -> Self {
        MemoryChunk {
            memory: vk::DeviceMemory::null(),
            range: ByteRange::new(0),
            _phantom: PhantomData,
        }
    }
}

pub struct MemoryBlock<M: MemoryProperties> {
    chunk: MemoryChunk<M>,
    page: Rc<RefCell<MemoryPage>>,
    ptr: Option<*mut c_void>,
}

impl<M: MemoryProperties> Debug for MemoryBlock<M> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("MemoryBlock")
            .field("chunk", &self.chunk)
            .field("page", &self.page)
            .field("ptr", &self.ptr)
            .finish()
    }
}

impl<M: MemoryProperties> From<&MemoryBlock<M>> for MemoryChunk<M> {
    fn from(block: &MemoryBlock<M>) -> Self {
        block.chunk
    }
}

pub trait HostVisibleMemory {
    fn map_memory(&mut self, device: &Device, range: ByteRange) -> Result<*mut c_void, vk::Result>;
    fn unmap_memory(&mut self, device: &Device);
}

impl HostVisibleMemory for MemoryBlock<HostCoherent> {
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

impl HostVisibleMemory for MemoryChunk<HostCoherent> {
    fn map_memory(&mut self, device: &Device, range: ByteRange) -> Result<*mut c_void, vk::Result> {
        unsafe {
            device.map_memory(
                self.memory,
                (self.range.beg + range.beg) as vk::DeviceSize,
                range.len() as vk::DeviceSize,
                vk::MemoryMapFlags::empty(),
            )
        }
    }

    fn unmap_memory(&mut self, device: &Device) {
        unsafe {
            device.unmap_memory(self.memory);
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
    pub fn try_allocate<M: MemoryProperties>(
        cell: &Rc<RefCell<Self>>,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<MemoryBlock<M>> {
        let mut page = cell.borrow_mut();
        if let Some(range) = page
            .alloc_range
            .alloc_raw(size as usize, alignment as usize)
        {
            Some(MemoryBlock {
                chunk: MemoryChunk {
                    memory: page.memory,
                    range,
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
struct MemoryType {
    index: u32,
    page_size: vk::DeviceSize,
    pages: Vec<Rc<RefCell<MemoryPage>>>,
}

impl MemoryType {
    pub fn try_allocate<M: MemoryProperties>(
        &mut self,
        device: &Device,
        size: vk::DeviceSize,
        alignment: vk::DeviceSize,
    ) -> Option<MemoryBlock<M>> {
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

pub trait Memory<M: MemoryProperties>: 'static {
    fn chunk(&self) -> MemoryChunk<M>;
}

impl<M: MemoryProperties> Memory<M> for MemoryChunk<M> {
    fn chunk(&self) -> MemoryChunk<M> {
        *self
    }
}

impl<T: 'static + Debug, M: MemoryProperties> Memory<M> for T
where
    for<'a> &'a T: Into<MemoryChunk<M>>,
{
    fn chunk(&self) -> MemoryChunk<M> {
        self.into()
    }
}

pub trait Allocator: 'static {
    type Config: AllocatorConfig;
    type Allocation<M: MemoryProperties>: Memory<M> + Debug;

    fn new(device: &VulkanDevice, config: &VulkanRendererConfig) -> Self;

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &Device,
        properites: &PhysicalDeviceProperties,
        request: AllocReq<M>,
    ) -> Result<Self::Allocation<M>, DeviceAllocError>;

    fn free<M: MemoryProperties>(&mut self, device: &Device, allocation: &mut Self::Allocation<M>);

    fn destroy(&mut self, device: &Device);
}

pub trait AllocatorConfig {
    fn get(config: &VulkanRendererConfig) -> Self;
}

pub struct StaticStackAllocatorConfig {
    page_size: vk::DeviceSize,
}

impl AllocatorConfig for StaticStackAllocatorConfig {
    fn get(config: &VulkanRendererConfig) -> Self {
        Self {
            page_size: config.page_size,
        }
    }
}

#[derive(Debug)]
pub struct StaticStackAllocator {
    memory_types: Vec<MemoryType>,
}

impl StaticStackAllocator {
    pub fn create(
        properties: &PhysicalDeviceProperties,
        page_size: vk::DeviceSize,
    ) -> Result<StaticStackAllocator, Box<dyn Error>> {
        let memory_types = (0..properties.memory.memory_types.len() as u32)
            .map(|index| MemoryType {
                page_size,
                index: index as u32,
                pages: Vec::new(),
            })
            .collect();
        Ok(StaticStackAllocator { memory_types })
    }

    pub fn destroy(&mut self, device: &Device) {
        self.memory_types.drain(0..).for_each(|mut memory_type| {
            memory_type.pages.drain(0..).for_each(|page| unsafe {
                device.free_memory(page.borrow_mut().memory, None);
            })
        });
    }
}

impl Allocator for StaticStackAllocator {
    type Config = StaticStackAllocatorConfig;
    type Allocation<M: MemoryProperties> = MemoryBlock<M>;

    fn new(device: &VulkanDevice, config: &VulkanRendererConfig) -> Self {
        let config = Self::Config::get(config);
        StaticStackAllocator::create(&device.physical_device.properties, config.page_size).unwrap()
    }

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &Device,
        properties: &PhysicalDeviceProperties,
        request: AllocReq<M>,
    ) -> Result<Self::Allocation<M>, DeviceAllocError> {
        let memory_type_index = request
            .get_memory_type_index(properties)
            .ok_or(DeviceAllocError::UnsupportedMemoryType)?;
        self.memory_types[memory_type_index as usize]
            .try_allocate(
                device,
                request.requirements.size,
                request.requirements.alignment,
            )
            .ok_or(DeviceAllocError::OutOfMemory)
    }

    fn free<M: MemoryProperties>(
        &mut self,
        _device: &Device,
        _allocation: &mut Self::Allocation<M>,
    ) {
    }

    fn destroy(&mut self, device: &Device) {
        self.destroy(device);
    }
}

pub struct DefaultAllocator {}

impl AllocatorConfig for Nil {
    fn get(_config: &VulkanRendererConfig) -> Self {
        Self {}
    }
}

impl Allocator for DefaultAllocator {
    type Config = Nil;
    type Allocation<M: MemoryProperties> = MemoryChunk<M>;

    fn new(_device: &VulkanDevice, _config: &VulkanRendererConfig) -> Self {
        DefaultAllocator {}
    }

    fn allocate<M: MemoryProperties>(
        &mut self,
        device: &Device,
        properties: &PhysicalDeviceProperties,
        request: AllocReq<M>,
    ) -> Result<Self::Allocation<M>, DeviceAllocError> {
        let memory_type_index = request
            .get_memory_type_index(properties)
            .ok_or(DeviceAllocError::UnsupportedMemoryType)?;
        let memory = unsafe {
            device.allocate_memory(
                &vk::MemoryAllocateInfo {
                    allocation_size: request.requirements.size,
                    memory_type_index,
                    ..Default::default()
                },
                None,
            )?
        };
        Ok(MemoryChunk {
            memory,
            range: ByteRange::new(request.requirements.size as usize),
            _phantom: PhantomData,
        })
    }

    fn free<M: MemoryProperties>(&mut self, device: &Device, allocation: &mut Self::Allocation<M>) {
        unsafe {
            device.free_memory(allocation.memory, None);
        }
        *allocation = MemoryChunk::empty();
    }

    fn destroy(&mut self, _device: &Device) {}
}
