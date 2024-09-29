use ash::vk;
use bytemuck::{cast_slice_mut, AnyBitPattern, NoUninit};
use std::{
    any::{type_name, TypeId},
    borrow::BorrowMut,
    error::Error,
    ffi::c_void,
    marker::PhantomData,
    mem::size_of,
    ops::{Index, IndexMut},
    ptr::copy_nonoverlapping,
    usize,
};

use crate::renderer::vulkan::device::{
    command::{
        operation::{self, Operation},
        SubmitSemaphoreState,
    },
    memory::{
        AllocReq, Allocator, DefaultAllocator, DeviceLocal, HostCoherent, HostVisibleMemory,
        MemoryProperties,
    },
    VulkanDevice,
};

use super::{image::VulkanImage2D, FromPartial, Partial, PartialBuilder};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ByteRange {
    pub beg: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn empty() -> Self {
        Self { beg: 0, end: 0 }
    }

    pub fn new(size: usize) -> Self {
        Self { beg: 0, end: size }
    }

    fn align<T>(offset: usize) -> usize {
        let alignment = std::mem::align_of::<T>();
        ((offset + alignment - 1) / alignment) * alignment
    }

    fn align_raw(offset: usize, alignment: usize) -> usize {
        ((offset + alignment - 1) / alignment) * alignment
    }

    fn extend<T: AnyBitPattern>(&mut self, len: usize) -> ByteRange {
        let beg = ByteRange::align::<T>(self.end);
        let end = beg + len * size_of::<T>();
        self.end = end;
        ByteRange { beg, end }
    }

    pub fn extend_raw(&mut self, len: usize, alignment: usize) -> ByteRange {
        let beg = ByteRange::align_raw(self.end, alignment);
        let end = beg + len;
        self.end = end;
        ByteRange { beg, end }
    }

    pub fn take<T: AnyBitPattern>(&mut self, count: usize) -> Option<ByteRange> {
        let beg = ByteRange::align::<T>(self.beg);
        let end = beg + count * size_of::<T>();
        if end <= self.end {
            self.beg = end;
            Some(ByteRange { beg, end })
        } else {
            None
        }
    }

    pub fn alloc_raw(&mut self, size: usize, alignment: usize) -> Option<ByteRange> {
        let beg = ByteRange::align_raw(self.beg, alignment);
        let end = beg + size;
        if end <= self.end {
            self.beg = end;
            Some(ByteRange { beg, end })
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.end - self.beg
    }
}

impl<T: AnyBitPattern> From<Range<T>> for ByteRange {
    fn from(value: Range<T>) -> Self {
        let beg = value.first * size_of::<T>();
        Self {
            beg,
            end: beg + value.len * size_of::<T>(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Range<T: AnyBitPattern> {
    pub len: usize,
    pub first: usize,
    _phantom: PhantomData<T>,
}

impl<T: AnyBitPattern> From<ByteRange> for Range<T> {
    fn from(value: ByteRange) -> Self {
        debug_assert_eq!(
            value.beg % size_of::<T>(),
            0,
            "Invalid Range<u8> offset for Range<{}> type!",
            std::any::type_name::<T>()
        );
        debug_assert_eq!(
            (value.end - value.beg) % size_of::<T>(),
            0,
            "Invalid Range<u8> size for Range<{}> type!",
            std::any::type_name::<T>()
        );
        Self {
            first: value.beg / size_of::<T>(),
            len: (value.end - value.beg) / size_of::<T>(),
            _phantom: PhantomData,
        }
    }
}

impl<T: AnyBitPattern> Range<T> {
    fn alloc(&mut self, len: usize) -> Self {
        debug_assert!(len <= self.len, "Range alloc overflow!");
        let first = self.first;
        self.first += len;
        self.len -= len;
        Self {
            first,
            len,
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BufferInfo<'a> {
    pub size: usize,
    pub usage: vk::BufferUsageFlags,
    pub sharing_mode: vk::SharingMode,
    pub queue_families: &'a [u32],
}

pub struct BufferBuilder<'a, M: MemoryProperties> {
    pub info: BufferInfo<'a>,
    _phantom: PhantomData<M>,
}

impl<'a, M: MemoryProperties> BufferBuilder<'a, M> {
    pub fn new(info: BufferInfo<'a>) -> Self {
        Self {
            info,
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct Buffer<M: MemoryProperties, A: Allocator> {
    size: usize,
    buffer: vk::Buffer,
    memory: A::Allocation<M>,
}

impl<M: MemoryProperties, A: Allocator> Buffer<M, A> {
    pub fn handle(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

#[derive(Debug)]
pub struct BufferPartial<M: MemoryProperties> {
    size: usize,
    req: AllocReq<M>,
    buffer: vk::Buffer,
}

impl<M: MemoryProperties> Partial for BufferPartial<M> {
    type Memory = M;

    fn requirements(&self) -> AllocReq<Self::Memory> {
        self.req
    }
}

impl<'a, M: MemoryProperties> PartialBuilder for BufferBuilder<'a, M> {
    type Partial = BufferPartial<M>;

    fn prepare(self, device: &VulkanDevice) -> Result<Self::Partial, Box<dyn Error>> {
        let BufferBuilder {
            info:
                BufferInfo {
                    size,
                    usage,
                    sharing_mode,
                    queue_families,
                },
            ..
        } = self;
        let create_info = vk::BufferCreateInfo {
            usage,
            sharing_mode,
            size: size as u64,
            queue_family_index_count: queue_families.len() as u32,
            p_queue_family_indices: queue_families.as_ptr(),
            ..Default::default()
        };
        let buffer = unsafe { device.create_buffer(&create_info, None)? };
        let req = device.get_alloc_req(buffer);
        Ok(BufferPartial { size, req, buffer })
    }
}

impl<M: MemoryProperties, A: Allocator> FromPartial for Buffer<M, A> {
    type Partial<'a> = BufferPartial<M>;
    type Allocator = A;

    fn finalize<'a>(
        partial: Self::Partial<'a>,
        device: &VulkanDevice,
        allocator: &mut Self::Allocator,
    ) -> Result<Self, Box<dyn Error>> {
        let BufferPartial { size, buffer, req } = partial;
        let memory = allocator.allocate(device, req)?;
        device.bind_memory(buffer, &memory)?;
        Ok(Buffer {
            size,
            buffer,
            memory,
        })
    }
}

impl VulkanDevice {
    pub fn destroy_buffer<'a, M: MemoryProperties, A: Allocator>(
        &self,
        buffer: impl Into<&'a mut Buffer<M, A>>,
        allocator: &mut A,
    ) {
        let buffer = buffer.into();
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            allocator.free(self, &mut buffer.memory);
        }
    }
}

pub struct StagingBufferBuilder {
    range: ByteRange,
}

impl Default for StagingBufferBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl StagingBufferBuilder {
    pub fn new() -> Self {
        Self {
            range: ByteRange::empty(),
        }
    }

    pub fn append<T: AnyBitPattern>(&mut self, len: usize) -> Range<T> {
        self.range.extend::<T>(len).into()
    }
}

pub struct StagingBuffer<'a> {
    range: ByteRange,
    buffer: PersistentBuffer<DefaultAllocator>,
    device: &'a VulkanDevice,
}

pub struct WritableRange<T: AnyBitPattern> {
    ptr: *mut T,
    range: Range<T>,
}

impl<'a, 'b> From<&'b StagingBuffer<'a>> for &'b Buffer<HostCoherent, DefaultAllocator> {
    fn from(value: &'b StagingBuffer) -> Self {
        (&value.buffer).into()
    }
}

impl<'a, 'b> From<&'b mut StagingBuffer<'a>> for &'b mut Buffer<HostCoherent, DefaultAllocator> {
    fn from(value: &'b mut StagingBuffer) -> Self {
        (&mut value.buffer).into()
    }
}

impl<'a> Drop for StagingBuffer<'a> {
    fn drop(&mut self) {
        self.device.destroy_buffer(self, &mut DefaultAllocator {});
    }
}

impl<'a> StagingBuffer<'a> {
    pub fn transfer_buffer_data<'b, D: Allocator>(
        &self,
        dst: impl Into<&'b mut Buffer<DeviceLocal, D>>,
        dst_offset: vk::DeviceSize,
    ) -> Result<(), Box<dyn Error>> {
        let command = self
            .device
            .allocate_transient_command::<operation::Transfer>()?;
        let command = self.device.begin_primary_command(command)?;
        let command = self.device.record_command(command, |command| {
            command.copy_buffer(
                &self.buffer,
                dst,
                &[vk::BufferCopy {
                    src_offset: 0,
                    dst_offset,
                    size: self.range.end as vk::DeviceSize,
                }],
            )
        });
        let command = self
            .device
            .submit_command(
                self.device.finish_command(command)?,
                SubmitSemaphoreState {
                    semaphores: &[],
                    masks: &[],
                },
                &[],
            )?
            .wait()?;
        self.device.free_command(&command);
        Ok(())
    }

    pub fn transfer_image_data<'b, A: Allocator>(
        &self,
        dst: impl Into<&'b mut VulkanImage2D<DeviceLocal, A>>,
        dst_array_layer: u32,
        dst_final_layout: vk::ImageLayout,
    ) -> Result<(), Box<dyn Error>> {
        let dst: &mut _ = dst.into();
        debug_assert!(
            dst.array_layers > dst_array_layer,
            "Invalid dst_array_layer for image data transfer!"
        );
        let dst_mip_levels = dst.mip_levels;
        let dst_old_layout = dst.layout;
        let command = self.device.begin_primary_command(
            self.device
                .allocate_transient_command::<operation::Graphics>()?,
        )?;
        let command = self.device.record_command(command, |command| {
            command
                .change_layout(
                    dst.borrow_mut(),
                    dst_old_layout,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    dst_array_layer,
                    0,
                    1,
                )
                .copy_image(self, dst.borrow_mut(), dst_array_layer)
                .generate_mip(dst.borrow_mut(), dst_array_layer)
                .change_layout(
                    dst.borrow_mut(),
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    dst_final_layout,
                    dst_array_layer,
                    0,
                    dst_mip_levels,
                )
        });

        let command = self
            .device
            .submit_command(
                self.device.finish_command(command)?,
                SubmitSemaphoreState {
                    semaphores: &[],
                    masks: &[],
                },
                &[],
            )?
            .wait()?;
        // Shouldn't free_command consume Command instead of taking it by reference?
        self.device.free_command(&command);
        Ok(())
    }

    pub fn write_range<T: AnyBitPattern>(&mut self, range: Range<T>) -> WritableRange<T> {
        // TODO: Improve safety,
        // - Range should comme from current staging buffer builder (unnecessary complexity?)
        debug_assert!(
            <Range<T> as Into<ByteRange>>::into(range).end <= self.range.end,
            "Invalid range for StagingBuffer write!"
        );
        WritableRange {
            range: Range {
                first: 0,
                len: range.len,
                _phantom: PhantomData,
            },
            ptr: unsafe { (self.buffer.ptr.unwrap() as *mut T).add(range.first) },
        }
    }
}

impl<T: AnyBitPattern> WritableRange<T> {
    pub fn write(&mut self, value: &[T]) -> Range<T> {
        let range = self.range.alloc(value.len());
        unsafe { copy_nonoverlapping(value.as_ptr(), self.ptr.add(range.first), value.len()) }
        range
    }
}

impl<T: AnyBitPattern + NoUninit> WritableRange<T> {
    pub fn remaining_as_slice_mut(&mut self) -> &mut [T] {
        let range = self.range.alloc(self.range.len);
        let values =
            unsafe { std::slice::from_raw_parts_mut::<T>(self.ptr.add(range.first), range.len) };
        cast_slice_mut(values)
    }
}

impl VulkanDevice {
    pub fn create_stagging_buffer(
        &self,
        builder: StagingBufferBuilder,
    ) -> Result<StagingBuffer, Box<dyn Error>> {
        let StagingBufferBuilder { range } = builder;
        let info = BufferInfo {
            size: range.end,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_families: &[operation::Transfer::get_queue_family_index(self)],
        };
        let partial = BufferBuilder::new(info).prepare(self)?;
        let buffer = PersistentBuffer::finalize(partial, self, &mut DefaultAllocator {})?;
        Ok(StagingBuffer {
            range,
            buffer,
            device: self,
        })
    }
}

pub struct PersistentBuffer<A: Allocator> {
    buffer: Buffer<HostCoherent, A>,
    ptr: Option<*mut c_void>,
}

impl<'a, A: Allocator> From<&'a PersistentBuffer<A>> for &'a Buffer<HostCoherent, A> {
    fn from(value: &'a PersistentBuffer<A>) -> Self {
        &value.buffer
    }
}

impl<'a, A: Allocator> From<&'a mut PersistentBuffer<A>> for &'a mut Buffer<HostCoherent, A> {
    fn from(value: &'a mut PersistentBuffer<A>) -> Self {
        &mut value.buffer
    }
}

impl<A: Allocator> FromPartial for PersistentBuffer<A>
where
    A::Allocation<HostCoherent>: HostVisibleMemory,
{
    type Partial<'a> = BufferPartial<HostCoherent>;
    type Allocator = A;

    fn finalize<'a>(
        partial: Self::Partial<'a>,
        device: &VulkanDevice,
        allocator: &mut Self::Allocator,
    ) -> Result<Self, Box<dyn Error>> {
        let mut buffer = Buffer::finalize(partial, device, allocator)?;
        let ptr = buffer.memory.map_memory(
            &device,
            ByteRange {
                beg: 0,
                end: buffer.size,
            },
        )?;
        Ok(PersistentBuffer {
            buffer,
            ptr: Some(ptr),
        })
    }
}

pub struct UniformBuffer<U: AnyBitPattern, O: Operation, A: Allocator> {
    len: usize,
    buffer: PersistentBuffer<A>,
    _phantom: PhantomData<(U, O)>,
}

pub struct UniformBufferPartial<U: AnyBitPattern, O: Operation> {
    len: usize,
    buffer: BufferPartial<HostCoherent>,
    _phantom: PhantomData<(U, O)>,
}

impl<U: AnyBitPattern, O: Operation> Partial for UniformBufferPartial<U, O> {
    type Memory = HostCoherent;

    fn requirements(&self) -> AllocReq<Self::Memory> {
        self.buffer.req
    }
}

pub struct UniformBufferBuilder<U: AnyBitPattern, O: Operation> {
    len: usize,
    _phantom: PhantomData<(U, O)>,
}

impl<U: AnyBitPattern, O: Operation> UniformBufferBuilder<U, O> {
    pub fn new(len: usize) -> Self {
        Self {
            len,
            _phantom: PhantomData,
        }
    }
}

impl<U: AnyBitPattern, O: Operation> PartialBuilder for UniformBufferBuilder<U, O> {
    type Partial = UniformBufferPartial<U, O>;

    fn prepare(self, device: &VulkanDevice) -> Result<Self::Partial, Box<dyn Error>> {
        let info = BufferInfo {
            size: size_of::<U>() * self.len,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            queue_families: &[O::get_queue_family_index(device)],
        };
        let buffer = BufferBuilder::new(info).prepare(device)?;
        Ok(UniformBufferPartial {
            len: self.len,
            buffer,
            _phantom: PhantomData,
        })
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> FromPartial for UniformBuffer<U, O, A>
where
    A::Allocation<HostCoherent>: HostVisibleMemory,
{
    type Partial<'a> = UniformBufferPartial<U, O>;
    type Allocator = A;

    fn finalize<'a>(
        partial: Self::Partial<'a>,
        device: &VulkanDevice,
        allocator: &mut Self::Allocator,
    ) -> Result<Self, Box<dyn Error>> {
        let len = partial.len;
        let buffer = PersistentBuffer::finalize(partial.buffer, device, allocator)?;
        Ok(UniformBuffer {
            len,
            buffer,
            _phantom: PhantomData,
        })
    }
}

impl<'a, U: AnyBitPattern, O: Operation, A: Allocator> From<&'a UniformBuffer<U, O, A>>
    for &'a Buffer<HostCoherent, A>
{
    fn from(value: &'a UniformBuffer<U, O, A>) -> Self {
        &value.buffer.buffer
    }
}

impl<'a, U: AnyBitPattern, O: Operation, A: Allocator> From<&'a mut UniformBuffer<U, O, A>>
    for &'a mut Buffer<HostCoherent, A>
{
    fn from(value: &'a mut UniformBuffer<U, O, A>) -> Self {
        &mut value.buffer.buffer
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> Index<usize> for UniformBuffer<U, O, A> {
    type Output = U;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.len, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_ref().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> IndexMut<usize> for UniformBuffer<U, O, A> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.len, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_mut().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> UniformBuffer<U, O, A> {
    pub fn handle(&self) -> vk::Buffer {
        self.buffer.buffer.handle()
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

// TODO: Move to separate module
pub struct UniformBufferTypeErased<O: Operation, A: Allocator> {
    len: usize,
    buffer: PersistentBuffer<A>,
    type_id: TypeId,
    _phantom: PhantomData<O>,
}

impl<P: AnyBitPattern, O: Operation, A: Allocator> From<UniformBuffer<P, O, A>>
    for UniformBufferTypeErased<O, A>
{
    fn from(value: UniformBuffer<P, O, A>) -> Self {
        let UniformBuffer { len, buffer, .. } = value;
        UniformBufferTypeErased {
            len,
            buffer,
            type_id: TypeId::of::<P>(),
            _phantom: PhantomData,
        }
    }
}

pub struct UniformBufferRef<'a, P: AnyBitPattern, O: Operation, A: Allocator> {
    len: usize,
    buffer: &'a mut PersistentBuffer<A>,
    _phantom: PhantomData<(P, O)>,
}

impl<'a, P: AnyBitPattern, O: Operation, A: Allocator>
    TryFrom<&'a mut UniformBufferTypeErased<O, A>> for UniformBufferRef<'a, P, O, A>
{
    type Error = Box<dyn Error>;

    fn try_from(value: &'a mut UniformBufferTypeErased<O, A>) -> Result<Self, Self::Error> {
        if value.type_id == TypeId::of::<P>() {
            Ok(UniformBufferRef {
                len: value.len,
                buffer: &mut value.buffer,
                _phantom: PhantomData,
            })
        } else {
            Err(format!(
                "Invalid uniform data type {} for uniform buffer!",
                type_name::<P>()
            ))?
        }
    }
}

impl<'a, O: Operation, A: Allocator> From<&'a mut UniformBufferTypeErased<O, A>>
    for &'a mut Buffer<HostCoherent, A>
{
    fn from(value: &'a mut UniformBufferTypeErased<O, A>) -> Self {
        (&mut value.buffer).into()
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> Index<usize> for UniformBufferRef<'_, U, O, A> {
    type Output = U;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.len, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_ref().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> IndexMut<usize>
    for UniformBufferRef<'_, U, O, A>
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.len, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_mut().unwrap() }
    }
}
