pub(super) mod buffer;
pub(super) mod command;
pub(super) mod image;
pub(super) mod pipeline;
pub(super) mod render_pass;
pub(super) mod resources;
pub(super) mod swapchain;

use self::command::{CommandPools, Operation};
use super::surface::{PhysicalDeviceSurfaceProperties, VulkanSurface};
use ash::{vk, Device, Instance};
use colored::Colorize;
use std::ffi::c_char;
use std::ops::Deref;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    ffi::CStr,
};
use swapchain::VulkanSwapchain;

#[derive(Debug, Clone, Copy)]
struct QueueFamilies {
    graphics: u32,
    compute: u32,
    transfer: u32,
}

impl QueueFamilies {
    pub fn get(
        properties: &PhysicalDeviceProperties,
        surface_properties: &PhysicalDeviceSurfaceProperties,
    ) -> Result<Self, Box<dyn Error>> {
        let mut queue_usages = HashMap::new();
        let mut try_use_queue_family = |queue: &mut Option<u32>, queue_family_index: u32| {
            if match queue {
                None => true,
                Some(current_index) if queue_usages[current_index] > 1 => {
                    queue_usages.entry(*current_index).and_modify(|n| *n -= 1);
                    true
                }
                _ => false,
            } {
                queue.replace(queue_family_index);
                queue_usages
                    .entry(queue_family_index)
                    .and_modify(|n| *n += 1)
                    .or_insert(1);
            }
        };
        let (mut graphics, mut compute, mut transfer) = (None, None, None);
        for &(properties, queue_family_index) in &properties.queue_families {
            if graphics.is_none()
                && properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && surface_properties
                    .supported_queue_families
                    .contains(&queue_family_index)
            {
                try_use_queue_family(&mut graphics, queue_family_index);
            }
            if properties.queue_flags.contains(vk::QueueFlags::COMPUTE) {
                try_use_queue_family(&mut compute, queue_family_index);
            }
            if transfer.is_none() && properties.queue_flags.contains(vk::QueueFlags::TRANSFER) {
                try_use_queue_family(&mut transfer, queue_family_index);
            }
        }
        Ok(Self {
            graphics: graphics.ok_or("Missing graphics queue family index!")?,
            compute: compute.ok_or("Missing compute queue family index!")?,
            transfer: transfer.ok_or("Missing transfer queue family index!")?,
        })
    }
}

struct DeviceQueueBuilder {
    queue_families: QueueFamilies,
    unique: HashSet<u32>,
}

impl DeviceQueueBuilder {
    pub fn new(queue_families: QueueFamilies) -> Self {
        let unique = HashSet::<u32>::from_iter([
            queue_families.compute,
            queue_families.graphics,
            queue_families.transfer,
        ]);
        Self {
            queue_families,
            unique,
        }
    }

    pub fn get_device_queue_create_infos(&self) -> Vec<vk::DeviceQueueCreateInfo> {
        self.unique
            .iter()
            .map(|&queue_family_index| vk::DeviceQueueCreateInfo {
                queue_family_index,
                queue_count: 1,
                p_queue_priorities: &1.0f32,
                ..Default::default()
            })
            .collect()
    }

    pub fn get_device_queues(&self, device: &Device) -> DeviceQueues {
        let quque_map =
            HashMap::<u32, vk::Queue>::from_iter(self.unique.iter().map(|&queue_family_index| {
                (queue_family_index, unsafe {
                    device.get_device_queue(queue_family_index, 0)
                })
            }));
        DeviceQueues {
            graphics: quque_map[&self.queue_families.graphics],
            compute: quque_map[&self.queue_families.compute],
            transfer: quque_map[&self.queue_families.transfer],
        }
    }
}

struct PhysicalDeviceProperties {
    features: vk::PhysicalDeviceFeatures,
    generic: vk::PhysicalDeviceProperties,
    memory: vk::PhysicalDeviceMemoryProperties,
    enabled_extension_names: Vec<*const c_char>,
    queue_families: Vec<(vk::QueueFamilyProperties, u32)>,
}

impl PhysicalDeviceProperties {
    pub fn get(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Self, Box<dyn Error>> {
        let generic = unsafe { instance.get_physical_device_properties(physical_device) };
        let features = unsafe { instance.get_physical_device_features(physical_device) };
        let memory = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        if generic.device_type != vk::PhysicalDeviceType::DISCRETE_GPU
            && generic.device_type != vk::PhysicalDeviceType::INTEGRATED_GPU
        {
            Err("Physical Device is not one of Discrete or Integrated GPU type!")?;
        }
        let enabled_extension_names =
            Self::check_required_device_extension_support(instance, physical_device)?;
        let queue_families = Self::get_device_queue_families_properties(instance, physical_device);
        Ok(Self {
            features,
            memory,
            generic,
            enabled_extension_names,
            queue_families,
        })
    }

    fn check_required_device_extension_support(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Vec<*const c_char>, Box<dyn Error>> {
        let supported_extensions =
            unsafe { instance.enumerate_device_extension_properties(physical_device)? };
        let required_extensions = VulkanSwapchain::required_extensions();
        let enabled_extension_names =
            required_extensions
                .iter()
                .try_fold(Vec::new(), |mut supported, req| {
                    supported_extensions
                    .iter()
                    .find(|sup| unsafe { CStr::from_ptr(&sup.extension_name as *const _) } == *req)
                    .is_some()
                    .then(|| {
                        supported.push(req.as_ptr());
                        supported
                    })
                    .ok_or(format!(
                        "Required device extension {} not suported!",
                        req.to_string_lossy()
                    ))
                })?;
        Ok(enabled_extension_names)
    }

    fn get_device_queue_families_properties(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Vec<(vk::QueueFamilyProperties, u32)> {
        let mut quque_properties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) }
                .into_iter()
                .zip(0 as u32..)
                .collect::<Vec<_>>();
        quque_properties.sort_by_key(|(properties, _)| {
            [
                vk::QueueFlags::GRAPHICS,
                vk::QueueFlags::COMPUTE,
                vk::QueueFlags::TRANSFER,
            ]
            .iter()
            .fold(0, |n, &flag| {
                properties
                    .queue_flags
                    .contains(flag)
                    .then_some(n + 1)
                    .unwrap_or(n)
            })
        });
        quque_properties
    }
}

struct AttachmentFormats {
    depth_stencil: vk::Format,
}

impl AttachmentFormats {
    const PREFERRED_DEPTH_FORMATS: &'static [vk::Format] = &[
        vk::Format::D32_SFLOAT_S8_UINT,
        vk::Format::D24_UNORM_S8_UINT,
        vk::Format::D16_UNORM_S8_UINT,
    ];

    pub fn get(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Self, Box<dyn Error>> {
        let depth_stencil = *Self::PREFERRED_DEPTH_FORMATS
            .iter()
            .find(|&&pref| {
                let format_properties = unsafe {
                    instance.get_physical_device_format_properties(physical_device, pref)
                };
                format_properties
                    .optimal_tiling_features
                    .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
            })
            .ok_or("No preffered format supported for Depth Stencil Attachment!")?;
        Ok(Self { depth_stencil })
    }
}

struct VulkanPhysicalDevice {
    properties: PhysicalDeviceProperties,
    surface_properties: PhysicalDeviceSurfaceProperties,
    attachment_formats: AttachmentFormats,
    queue_families: QueueFamilies,
    handle: vk::PhysicalDevice,
}

struct DeviceQueues {
    graphics: vk::Queue,
    compute: vk::Queue,
    transfer: vk::Queue,
}

pub(super) struct VulkanDevice {
    physical_device: VulkanPhysicalDevice,
    coomand_pools: CommandPools,
    device_queues: DeviceQueues,
    device: Device,
}

impl Deref for VulkanDevice {
    type Target = Device;
    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

fn check_physical_device_suitable(
    physical_device: vk::PhysicalDevice,
    instance: &Instance,
    surface: &VulkanSurface,
) -> Result<VulkanPhysicalDevice, Box<dyn Error>> {
    let properties = PhysicalDeviceProperties::get(instance, physical_device)?;
    let surface_properties =
        PhysicalDeviceSurfaceProperties::get(surface, physical_device, &properties.queue_families)?;
    let attachment_formats = AttachmentFormats::get(instance, physical_device)?;
    let queue_families = QueueFamilies::get(&properties, &surface_properties)?;
    Ok(VulkanPhysicalDevice {
        properties,
        surface_properties,
        attachment_formats,
        queue_families,
        handle: physical_device,
    })
}

fn get_physical_device_name(instance: &Instance, physical_device: vk::PhysicalDevice) -> String {
    unsafe {
        CStr::from_ptr(
            &instance
                .get_physical_device_properties(physical_device)
                .device_name as *const c_char,
        )
    }
    .to_string_lossy()
    .into_owned()
}

fn pick_physical_device(
    instance: &Instance,
    surface: &VulkanSurface,
) -> Result<VulkanPhysicalDevice, Box<dyn Error>> {
    let (physical_device_name, physical_device) = unsafe { instance.enumerate_physical_devices()? }
        .into_iter()
        .find_map(|physical_device| {
            let device_name = get_physical_device_name(instance, physical_device);
            match check_physical_device_suitable(physical_device, instance, surface) {
                Ok(physical_device) => Some((device_name, physical_device)),
                Err(error) => {
                    println!(
                        "{} PhysicalDevice not suitable: {}",
                        device_name.red(),
                        error
                    );
                    None
                }
            }
        })
        .ok_or("Failed to pick suitable physical device!")?;
    println!("Using {} Physical Device", physical_device_name.green());
    Ok(physical_device)
}

impl VulkanDevice {
    pub fn create(
        instance: &Instance,
        surface: &VulkanSurface,
    ) -> Result<VulkanDevice, Box<dyn Error>> {
        let physical_device = pick_physical_device(instance, surface)?;
        let queue_builder = DeviceQueueBuilder::new(physical_device.queue_families);
        let device = unsafe {
            instance.create_device(
                physical_device.handle,
                &vk::DeviceCreateInfo::builder()
                    .queue_create_infos(&queue_builder.get_device_queue_create_infos())
                    .enabled_extension_names(&physical_device.properties.enabled_extension_names),
                None,
            )?
        };
        let device_queues = queue_builder.get_device_queues(&device);
        let coomand_pools = CommandPools::create(&device, physical_device.queue_families)?;
        Ok(Self {
            physical_device,
            coomand_pools,
            device_queues,
            device,
        })
    }

    pub fn destory(&mut self) {
        unsafe {
            self.coomand_pools.destory(&self.device);
            self.device.destroy_device(None);
        }
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

    pub fn wait_idle(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            self.device.device_wait_idle()?;
        }
        Ok(())
    }

    pub fn get_queue_families(&self, operations: &[Operation]) -> Vec<u32> {
        Vec::from_iter(HashSet::<u32>::from_iter(operations.iter().map(
            |&operation| match operation {
                Operation::Graphics => self.physical_device.queue_families.graphics,
                Operation::Compute => self.physical_device.queue_families.compute,
                Operation::Transfer => self.physical_device.queue_families.transfer,
            },
        )))
    }
}
