use std::{
    ffi::{CString, c_void},
    str::FromStr as _,
    sync::Arc,
};

use bevy_ecs::resource::Resource;
use connect_shared::BufferKey;
use slotmap::SlotMap;
use vulkan::{Device, Instance, vk::*};
use vulkan_vma::*;

// TODO: MOVE TO SOME PLACE

#[derive(Clone, Copy)]
pub struct CommandGroup {
    pub command_pool: CommandPool,
    pub command_buffer: CommandBuffer,
    pub fence: Fence,
}

//////////////////////////////////////////////////

pub struct MapppedAllocationHandler {
    allocator: Allocator,
    allocation: Allocation,
    ptr: *mut u8,
}

impl MapppedAllocationHandler {
    pub fn new(allocator: Allocator, allocation: Allocation, ptr: *mut u8) -> Self {
        Self {
            allocator,
            allocation,
            ptr,
        }
    }

    pub fn get_ptr(&self) -> *mut u8 {
        self.ptr
    }
}

impl Drop for MapppedAllocationHandler {
    fn drop(&mut self) {
        unsafe {
            self.allocator.unmap_memory(self.allocation);
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum BufferVisibility {
    #[default]
    Unspecified,
    HostVisible,
    DeviceOnly,
}

pub struct AllocatedBuffer {
    pub buffer: Buffer,
    pub allocation: Allocation,
    pub buffer_info: BufferInfo,
}

#[derive(Default, Clone, Copy)]
pub struct BufferReference {
    key: BufferKey,
    buffer_info: BufferInfo,
}

#[derive(Default, Clone, Copy)]
pub struct BufferInfo {
    pub device_address: DeviceAddress,
    pub size: DeviceSize,
    pub buffer_visibility: BufferVisibility,
}

impl BufferInfo {
    pub fn new(
        device_address: DeviceAddress,
        size: DeviceSize,
        buffer_visibility: BufferVisibility,
    ) -> Self {
        Self {
            device_address,
            size,
            buffer_visibility,
        }
    }
}

impl BufferReference {
    pub fn get_buffer<'a>(
        &'a self,
        buffers_pool: &'a BuffersPoolResource,
    ) -> Option<&'a AllocatedBuffer> {
        buffers_pool.get_buffer(*self)
    }

    pub fn get_buffer_info(&self) -> BufferInfo {
        self.buffer_info
    }
}

#[derive(Resource)]
pub struct BuffersPoolResource {
    instance: Arc<Instance>,
    device: Arc<Device>,
    allocator: Allocator,
    slots: SlotMap<BufferKey, AllocatedBuffer>,
    staging_buffer_reference: BufferReference,
    upload_command_group: CommandGroup,
    transfer_queue: Queue,
}

impl BuffersPoolResource {
    pub fn new(
        instance: Arc<Instance>,
        device: Arc<Device>,
        allocator: Allocator,
        upload_command_group: CommandGroup,
        transfer_queue: Queue,
    ) -> Self {
        let mut memory_bucket = Self {
            instance,
            device,
            allocator,
            slots: SlotMap::with_capacity_and_key(2_048),
            staging_buffer_reference: Default::default(),
            upload_command_group,
            transfer_queue,
        };

        // Pre-allocate 64 MB for transfers.
        let staging_buffer_reference = memory_bucket.create_buffer(
            1024 * 1024 * 64,
            BufferUsageFlags::TRANSFER_SRC,
            BufferVisibility::HostVisible,
            None,
            Some("Staging Buffer".to_string()),
        );
        memory_bucket.staging_buffer_reference = staging_buffer_reference;

        memory_bucket
    }

    pub fn create_buffer(
        &mut self,
        allocation_size: usize,
        usage: BufferUsageFlags,
        buffer_visibility: BufferVisibility,
        memory_property_flags: Option<MemoryPropertyFlags>,
        name: Option<String>,
    ) -> BufferReference {
        let mut buffer_kind_usage = if allocation_size < 1024 * 64 {
            BufferUsageFlags::UNIFORM_BUFFER
        } else {
            BufferUsageFlags::STORAGE_BUFFER
        };

        if usage.contains(BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT) {
            buffer_kind_usage = BufferUsageFlags::empty();
        }

        let buffer_create_info = BufferCreateInfo {
            size: allocation_size as _,
            usage: usage | buffer_kind_usage | BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            sharing_mode: SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        if buffer_visibility == BufferVisibility::Unspecified {
            panic!("Trying to create a buffer with unspecified visibility!");
        }

        let allocation_flags = match buffer_visibility {
            BufferVisibility::HostVisible => {
                AllocationCreateFlags::MAPPED
                    | AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE
                    | AllocationCreateFlags::STRATEGY_MIN_MEMORY
            }
            BufferVisibility::DeviceOnly => AllocationCreateFlags::STRATEGY_MIN_MEMORY,
            BufferVisibility::Unspecified => unreachable!(),
        };

        let preferred_flags = match buffer_visibility {
            BufferVisibility::HostVisible => MemoryPropertyFlags::HOST_COHERENT,
            BufferVisibility::DeviceOnly => MemoryPropertyFlags::empty(),
            BufferVisibility::Unspecified => unreachable!(),
        };

        let additional_memory_property_flags = if buffer_visibility == BufferVisibility::HostVisible
        {
            MemoryPropertyFlags::empty()
        } else {
            match memory_property_flags {
                Some(memory_property_flags) => memory_property_flags,
                None => MemoryPropertyFlags::empty(),
            }
        };

        let base_required_flags = match buffer_visibility {
            BufferVisibility::HostVisible => MemoryPropertyFlags::empty(),
            BufferVisibility::DeviceOnly => MemoryPropertyFlags::DEVICE_LOCAL,
            BufferVisibility::Unspecified => unreachable!(),
        };

        let required_flags = base_required_flags | additional_memory_property_flags;

        let allocation_create_info = AllocationOptions {
            flags: allocation_flags,
            usage: MemoryUsage::Auto,
            required_flags,
            preferred_flags,
            ..Default::default()
        };

        let (buffer, allocation) = unsafe {
            self.allocator
                .create_buffer(buffer_create_info, &allocation_create_info)
                .unwrap()
        };
        let device_address = unsafe { self.get_device_address(buffer) };

        if let Some(name) = name {
            let name = CString::from_str(name.as_str()).unwrap();
            let debug_utils_object_name = DebugUtilsObjectNameInfoEXT {
                object_type: ObjectType::BUFFER,
                object_handle: buffer.as_raw(),
                object_name: name.as_ptr() as *const _,
                ..Default::default()
            };

            unsafe {
                self.instance
                    .set_debug_utils_object_name_ext(self.device.handle(), &debug_utils_object_name)
                    .unwrap();
            }
        }

        let buffer_info = BufferInfo::new(device_address, allocation_size as _, buffer_visibility);
        let allocated_buffer = AllocatedBuffer {
            buffer,
            allocation,
            buffer_info,
        };

        self.insert_buffer(allocated_buffer)
    }

    fn insert_buffer(&mut self, allocated_buffer: AllocatedBuffer) -> BufferReference {
        let buffer_info = allocated_buffer.buffer_info;
        let buffer_key = self.slots.insert(allocated_buffer);

        BufferReference {
            key: buffer_key,
            buffer_info,
        }
    }

    pub fn get_buffer(&self, buffer_reference: BufferReference) -> Option<&AllocatedBuffer> {
        self.slots.get(buffer_reference.key)
    }

    unsafe fn get_device_address(&self, buffer: Buffer) -> DeviceAddress {
        let buffer_device_address = BufferDeviceAddressInfoBuilder::default()
            .buffer(buffer)
            .build();

        unsafe {
            self.device
                .get_buffer_device_address(&buffer_device_address)
        }
    }

    pub unsafe fn transfer_data_to_buffer(
        &self,
        buffer_reference: BufferReference,
        src: &[u8],
        size: usize,
    ) {
        let allocated_buffer = buffer_reference.get_buffer(self).unwrap();

        let buffer_visibility = allocated_buffer.buffer_info.buffer_visibility;
        let target_buffer = match buffer_visibility {
            BufferVisibility::HostVisible => allocated_buffer,
            BufferVisibility::DeviceOnly => self.get_buffer(self.staging_buffer_reference).unwrap(),
            BufferVisibility::Unspecified => unreachable!(),
        };

        unsafe {
            let p_mapped_memory = self.allocator.map_memory(target_buffer.allocation).unwrap();

            std::ptr::copy_nonoverlapping(src.as_ptr(), p_mapped_memory as _, size);

            self.allocator.unmap_memory(target_buffer.allocation);
        }

        if buffer_visibility == BufferVisibility::DeviceOnly {
            let regions_to_copy = [BufferCopy {
                size: size as _,
                ..Default::default()
            }];
            unsafe {
                self.copy_buffer_to_buffer(
                    target_buffer.buffer,
                    allocated_buffer.buffer,
                    &regions_to_copy,
                )
            }
        }
    }

    pub fn get_staging_buffer_reference(&self) -> BufferReference {
        self.staging_buffer_reference
    }

    pub unsafe fn transfer_data_to_buffer_raw(
        &mut self,
        buffer_reference: BufferReference,
        src: *const c_void,
        size: usize,
    ) {
        let allocated_buffer = buffer_reference.get_buffer(self).unwrap();

        let buffer_visibility = allocated_buffer.buffer_info.buffer_visibility;
        let target_buffer = match buffer_visibility {
            BufferVisibility::HostVisible => allocated_buffer,
            BufferVisibility::DeviceOnly => self.get_buffer(self.staging_buffer_reference).unwrap(),
            BufferVisibility::Unspecified => unreachable!(),
        };

        unsafe {
            let p_mapped_memory = self.allocator.map_memory(target_buffer.allocation).unwrap();

            std::ptr::copy_nonoverlapping(src, p_mapped_memory as _, size);

            self.allocator.unmap_memory(target_buffer.allocation);
        }

        if buffer_visibility == BufferVisibility::DeviceOnly {
            let regions_to_copy = [BufferCopy {
                size: size as _,
                ..Default::default()
            }];
            unsafe {
                self.copy_buffer_to_buffer(
                    target_buffer.buffer,
                    allocated_buffer.buffer,
                    &regions_to_copy,
                )
            }
        }
    }

    pub unsafe fn transfer_data_to_buffer_with_offset(
        &self,
        buffer_reference: BufferReference,
        src: *const c_void,
        regions_to_copy: &[BufferCopy],
    ) {
        let allocated_buffer = buffer_reference.get_buffer(self).unwrap();

        let buffer_visibility = allocated_buffer.buffer_info.buffer_visibility;
        let target_buffer = match buffer_visibility {
            BufferVisibility::HostVisible => allocated_buffer,
            BufferVisibility::DeviceOnly => self.get_buffer(self.staging_buffer_reference).unwrap(),
            BufferVisibility::Unspecified => unreachable!(),
        };

        let mut gpu_copy_regions = Vec::with_capacity(regions_to_copy.len());

        unsafe {
            let ptr_mapped_memory = self.allocator.map_memory(target_buffer.allocation).unwrap();
            let mut staging_write_offset = Default::default();

            for &buffer_copy in regions_to_copy {
                let src_with_offset = src.add(buffer_copy.src_offset as usize);

                let cpu_write_offset = if buffer_visibility == BufferVisibility::HostVisible {
                    buffer_copy.dst_offset as usize
                } else {
                    staging_write_offset
                };

                let ptr_mapped_memory_with_offset = ptr_mapped_memory.add(cpu_write_offset);

                std::ptr::copy_nonoverlapping(
                    src_with_offset,
                    ptr_mapped_memory_with_offset as _,
                    buffer_copy.size as usize,
                );

                if buffer_visibility == BufferVisibility::DeviceOnly {
                    let mut gpu_region = buffer_copy;
                    gpu_region.src_offset = staging_write_offset as u64;
                    gpu_copy_regions.push(gpu_region);

                    staging_write_offset += buffer_copy.size as usize;
                }
            }

            self.allocator.unmap_memory(target_buffer.allocation);
        }

        if buffer_visibility == BufferVisibility::DeviceOnly {
            unsafe {
                self.copy_buffer_to_buffer(
                    target_buffer.buffer,
                    allocated_buffer.buffer,
                    &gpu_copy_regions,
                )
            }
        }
    }

    unsafe fn copy_buffer_to_buffer(
        &self,
        src_buffer: Buffer,
        dst_buffer: Buffer,
        regions_to_copy: &[BufferCopy],
    ) {
        let command_buffer = self.upload_command_group.command_buffer;

        let command_buffer_begin_info = CommandBufferBeginInfo {
            flags: CommandBufferUsageFlags::ONE_TIME_SUBMIT,
            ..Default::default()
        };

        unsafe {
            self.device
                .begin_command_buffer(command_buffer, &command_buffer_begin_info)
                .unwrap();
        }

        unsafe {
            self.device.cmd_copy_buffer(
                self.upload_command_group.command_buffer,
                src_buffer,
                dst_buffer,
                regions_to_copy,
            );
        }

        unsafe {
            self.device.end_command_buffer(command_buffer).unwrap();
        }

        let command_buffers = [command_buffer];
        let queue_submits =
            [SubmitInfoBuilder::default().command_buffers(command_buffers.as_slice())];

        unsafe {
            self.device
                .queue_submit(
                    self.transfer_queue,
                    &queue_submits,
                    self.upload_command_group.fence,
                )
                .unwrap();
        }

        let fences_to_wait = [self.upload_command_group.fence];

        unsafe {
            self.device
                .wait_for_fences(fences_to_wait.as_slice(), true, u64::MAX)
                .unwrap();
            self.device.reset_fences(fences_to_wait.as_slice()).unwrap();

            self.device
                .reset_command_pool(
                    self.upload_command_group.command_pool,
                    CommandPoolResetFlags::RELEASE_RESOURCES,
                )
                .unwrap();
        }
    }

    pub fn map_allocation(&self, buffer_reference: BufferReference) -> MapppedAllocationHandler {
        let allocated_buffer = self.get_buffer(buffer_reference).unwrap();

        let ptr = unsafe {
            self.allocator
                .map_memory(allocated_buffer.allocation)
                .unwrap()
        };

        MapppedAllocationHandler::new(self.allocator, allocated_buffer.allocation, ptr)
    }

    pub unsafe fn free_allocations(&mut self) {
        self.slots.drain().for_each(|(_, allocated_buffer)| unsafe {
            self.allocator
                .destroy_buffer(allocated_buffer.buffer, allocated_buffer.allocation);
        });
    }
}
