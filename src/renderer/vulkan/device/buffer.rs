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

use super::{
    command::{
        operation::{self, Operation},
        SubmitSemaphoreState,
    },
    image::VulkanImage2D,
    memory::{
        Allocator, DefaultAllocator, DeviceLocal, HostCoherent, HostVisibleMemory, MemoryProperties,
    },
    VulkanDevice,
};

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

    pub fn take<T: AnyBitPattern>(&mut self, count: usize) -> Option<ByteRange> {
        let beg = ByteRange::align::<T>(self.beg);
        let end = beg + count * size_of::<T>();
        if end < self.end {
            self.beg = end;
            Some(ByteRange { beg, end })
        } else {
            None
        }
    }

    pub fn alloc_raw(&mut self, size: usize, alignment: usize) -> Option<ByteRange> {
        let beg = ByteRange::align_raw(self.beg, alignment);
        let end = beg + size;
        if end < self.end {
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

#[derive(Debug)]
pub struct Buffer<M: MemoryProperties, A: Allocator> {
    pub size: usize,
    pub buffer: vk::Buffer,
    memory: A::Allocation<M>,
}

impl VulkanDevice {
    pub fn create_buffer<M: MemoryProperties, A: Allocator>(
        &self,
        allocator: &mut A,
        info: BufferInfo,
    ) -> Result<Buffer<M, A>, Box<dyn Error>> {
        let BufferInfo {
            size,
            usage,
            sharing_mode,
            queue_families,
        } = info;
        let create_info = vk::BufferCreateInfo {
            usage,
            sharing_mode,
            size: size as u64,
            queue_family_index_count: queue_families.len() as u32,
            p_queue_family_indices: queue_families.as_ptr(),
            ..Default::default()
        };
        let (buffer, memory) = unsafe {
            let buffer = self.device.create_buffer(&create_info, None)?;
            let memory = allocator.allocate(
                &self.device,
                &self.physical_device.properties,
                self.get_alloc_req::<_, M>(buffer),
            )?;
            self.bind_memory(buffer, &memory)?;
            (buffer, memory)
        };
        Ok(Buffer {
            size,
            buffer,
            memory,
        })
    }

    pub fn destroy_buffer<M: MemoryProperties, A: Allocator>(
        &self,
        buffer: &mut Buffer<M, A>,
        allocator: &mut A,
    ) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            allocator.free(&self.device, &mut buffer.memory);
        }
    }
}

pub struct HostVisibleBuffer<A: Allocator> {
    buffer: Buffer<HostCoherent, A>,
}

impl<'a, A: Allocator> From<&'a HostVisibleBuffer<A>> for &'a Buffer<HostCoherent, A> {
    fn from(value: &'a HostVisibleBuffer<A>) -> Self {
        &value.buffer
    }
}

impl<'a, A: Allocator> From<&'a mut HostVisibleBuffer<A>> for &'a mut Buffer<HostCoherent, A> {
    fn from(value: &'a mut HostVisibleBuffer<A>) -> Self {
        &mut value.buffer
    }
}

impl VulkanDevice {
    pub fn create_host_visible_buffer<A: Allocator>(
        &self,
        allocator: &mut A,
        info: BufferInfo,
    ) -> Result<HostVisibleBuffer<A>, Box<dyn Error>> {
        let buffer = self.create_buffer(allocator, info)?;
        Ok(HostVisibleBuffer { buffer })
    }
}

#[derive(Debug)]
pub struct DeviceLocalBuffer<A: Allocator> {
    pub buffer: Buffer<DeviceLocal, A>,
}

impl<'a, A: Allocator> From<&'a DeviceLocalBuffer<A>> for &'a Buffer<DeviceLocal, A> {
    fn from(value: &'a DeviceLocalBuffer<A>) -> Self {
        &value.buffer
    }
}

impl<'a, A: Allocator> From<&'a mut DeviceLocalBuffer<A>> for &'a mut Buffer<DeviceLocal, A> {
    fn from(value: &'a mut DeviceLocalBuffer<A>) -> Self {
        &mut value.buffer
    }
}

impl VulkanDevice {
    pub fn create_device_local_buffer<A: Allocator>(
        &self,
        allocator: &mut A,
        info: BufferInfo,
    ) -> Result<DeviceLocalBuffer<A>, Box<dyn Error>> {
        let buffer = self.create_buffer(allocator, info)?;
        Ok(DeviceLocalBuffer { buffer })
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

impl<'a> From<&'a StagingBuffer<'a>> for &'a Buffer<HostCoherent, DefaultAllocator> {
    fn from(value: &'a StagingBuffer) -> Self {
        (&value.buffer).into()
    }
}

impl<'a> From<&'a mut StagingBuffer<'a>> for &'a mut Buffer<HostCoherent, DefaultAllocator> {
    fn from(value: &'a mut StagingBuffer) -> Self {
        (&mut value.buffer).into()
    }
}

impl<'a> Drop for StagingBuffer<'a> {
    fn drop(&mut self) {
        self.device
            .destroy_persistent_buffer(&mut self.buffer, &mut DefaultAllocator {});
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
        let buffer = self.create_persistent_buffer(
            &mut DefaultAllocator {},
            BufferInfo {
                size: range.end,
                usage: vk::BufferUsageFlags::TRANSFER_SRC,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                queue_families: &[operation::Transfer::get_queue_family_index(self)],
            },
        )?;
        Ok(StagingBuffer {
            range,
            buffer,
            device: self,
        })
    }
}

pub struct PersistentBuffer<A: Allocator> {
    buffer: HostVisibleBuffer<A>,
    ptr: Option<*mut c_void>,
}

impl<'a, A: Allocator> From<&'a PersistentBuffer<A>> for &'a Buffer<HostCoherent, A> {
    fn from(value: &'a PersistentBuffer<A>) -> Self {
        (&value.buffer).into()
    }
}

impl<'a, A: Allocator> From<&'a mut PersistentBuffer<A>> for &'a mut Buffer<HostCoherent, A> {
    fn from(value: &'a mut PersistentBuffer<A>) -> Self {
        (&mut value.buffer).into()
    }
}

impl VulkanDevice {
    pub fn create_persistent_buffer<A: Allocator>(
        &self,
        allcator: &mut A,
        info: BufferInfo,
    ) -> Result<PersistentBuffer<A>, Box<dyn Error>>
    where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let mut buffer = self.create_host_visible_buffer(allcator, info)?;
        let ptr = buffer.buffer.memory.map_memory(
            self,
            ByteRange {
                beg: 0,
                end: info.size,
            },
        )?;
        Ok(PersistentBuffer {
            buffer,
            ptr: Some(ptr),
        })
    }

    pub fn destroy_persistent_buffer<A: Allocator>(
        &self,
        buffer: &mut PersistentBuffer<A>,
        allocator: &mut A,
    ) where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        buffer.buffer.buffer.memory.unmap_memory(self);
        self.destroy_buffer(buffer.into(), allocator);
    }
}

pub struct UniformBuffer<U: AnyBitPattern, O: Operation, A: Allocator> {
    buffer: PersistentBuffer<A>,
    pub size: usize,
    _phantom: PhantomData<(U, O)>,
}

impl<'a, U: AnyBitPattern, O: Operation, A: Allocator> From<&'a UniformBuffer<U, O, A>>
    for &'a PersistentBuffer<A>
{
    fn from(value: &'a UniformBuffer<U, O, A>) -> Self {
        &value.buffer
    }
}

impl<'a, U: AnyBitPattern, O: Operation, A: Allocator> From<&'a mut UniformBuffer<U, O, A>>
    for &'a mut PersistentBuffer<A>
{
    fn from(value: &'a mut UniformBuffer<U, O, A>) -> Self {
        &mut value.buffer
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> Index<usize> for UniformBuffer<U, O, A> {
    type Output = U;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.size, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_ref().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> IndexMut<usize> for UniformBuffer<U, O, A> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.size, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_mut().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> UniformBuffer<U, O, A> {
    pub fn as_raw(&self) -> vk::Buffer {
        // Do it more elegant way later, maybe push as_raw up the encapsulation chain?
        self.buffer.buffer.buffer.buffer
    }
}

impl VulkanDevice {
    pub(super) fn create_uniform_buffer<U: AnyBitPattern, O: Operation, A: Allocator>(
        &self,
        allocator: &mut A,
        size: usize,
    ) -> Result<UniformBuffer<U, O, A>, Box<dyn Error>>
    where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        let buffer = self.create_persistent_buffer(
            allocator,
            BufferInfo {
                size: size_of::<U>() * size,
                usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                queue_families: &[O::get_queue_family_index(self)],
            },
        )?;
        Ok(UniformBuffer {
            buffer,
            size,
            _phantom: PhantomData,
        })
    }

    pub(super) fn destroy_uniform_buffer<U: AnyBitPattern, O: Operation, A: Allocator>(
        &self,
        buffer: &mut UniformBuffer<U, O, A>,
        allocator: &mut A,
    ) where
        A::Allocation<HostCoherent>: HostVisibleMemory,
    {
        self.destroy_persistent_buffer(buffer.into(), allocator);
    }
}

// TODO: Move to separate module
pub struct UniformBufferTypeErased<O: Operation, A: Allocator> {
    type_id: TypeId,
    buffer: PersistentBuffer<A>,
    pub size: usize,
    _phantom: PhantomData<O>,
}

impl<P: AnyBitPattern, O: Operation, A: Allocator> From<UniformBuffer<P, O, A>>
    for UniformBufferTypeErased<O, A>
{
    fn from(value: UniformBuffer<P, O, A>) -> Self {
        let UniformBuffer { buffer, size, .. } = value;
        UniformBufferTypeErased {
            type_id: TypeId::of::<P>(),
            buffer,
            size,
            _phantom: PhantomData,
        }
    }
}

pub struct UniformBufferRef<'a, P: AnyBitPattern, O: Operation, A: Allocator> {
    buffer: &'a mut PersistentBuffer<A>,
    pub size: usize,
    _phantom: PhantomData<(P, O)>,
}

impl<'a, P: AnyBitPattern, O: Operation, A: Allocator>
    TryFrom<&'a mut UniformBufferTypeErased<O, A>> for UniformBufferRef<'a, P, O, A>
{
    type Error = Box<dyn Error>;

    fn try_from(value: &'a mut UniformBufferTypeErased<O, A>) -> Result<Self, Self::Error> {
        if value.type_id == TypeId::of::<P>() {
            Ok(UniformBufferRef {
                buffer: &mut value.buffer,
                size: value.size,
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
    for &'a mut PersistentBuffer<A>
{
    fn from(value: &'a mut UniformBufferTypeErased<O, A>) -> Self {
        &mut value.buffer
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> Index<usize> for UniformBufferRef<'_, U, O, A> {
    type Output = U;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.size, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_ref().unwrap() }
    }
}

impl<U: AnyBitPattern, O: Operation, A: Allocator> IndexMut<usize>
    for UniformBufferRef<'_, U, O, A>
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < self.size, "Out of range UniformBuffer access!");
        let ptr = self.buffer.ptr.unwrap() as *mut U;
        unsafe { ptr.add(index).as_mut().unwrap() }
    }
}
