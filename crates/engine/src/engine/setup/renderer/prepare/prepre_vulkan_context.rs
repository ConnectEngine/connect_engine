use std::{collections::HashSet, ffi::CStr};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle};
use vma::{Allocator, AllocatorCreateFlags, AllocatorCreateInfo};
use vulkanite::{
    DefaultAllocator, Dispatcher, DynamicDispatcher, flagbits, structure_chain,
    vk::{
        self, EXT_DESCRIPTOR_BUFFER, EXT_MESH_SHADER, EXT_SHADER_OBJECT, KHR_UNIFIED_IMAGE_LAYOUTS,
        PhysicalDeviceDescriptorBufferFeaturesEXT, PhysicalDeviceMeshShaderFeaturesEXT,
        PhysicalDeviceRobustness2FeaturesKHR, PhysicalDeviceShaderObjectFeaturesEXT,
        PhysicalDeviceUnifiedImageLayoutsFeaturesKHR, PhysicalDeviceVulkan11Features,
        PhysicalDeviceVulkan12Features, PhysicalDeviceVulkan13Features, SurfaceFormatKHR,
        ValidationFeatureEnableEXT, ValidationFeaturesEXT,
        rs::{PhysicalDevice, SwapchainKHR},
    },
    window,
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::engine::Engine;

use connect_renderer::*;

extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    ty: vk::DebugUtilsMessageTypeFlagsEXT,
    data: &vk::DebugUtilsMessengerCallbackDataEXT,
    _: *const (),
) -> vk::Bool32 {
    use vk::DebugUtilsMessageSeverityFlagsEXT as Severity;
    use vk::DebugUtilsMessageTypeFlagsEXT as Type;

    let message = unsafe { CStr::from_ptr(data.p_message).to_string_lossy() };
    let trimmed = message.trim();

    static mut IN_DEVICE_SETUP: bool = false;
    static mut DEVICE_REPORTED: bool = false;

    if trimmed.contains("vkCreateDevice layer callstack setup to:") {
        unsafe {
            IN_DEVICE_SETUP = true;
        }
        return vk::FALSE;
    }

    if trimmed.starts_with("<Device>") {
        unsafe {
            IN_DEVICE_SETUP = false;
        }
        return vk::FALSE;
    }

    if unsafe { IN_DEVICE_SETUP } {
        return vk::FALSE;
    }

    if trimmed.contains("Inserted device layer") {
        return vk::FALSE;
    }

    if !unsafe { DEVICE_REPORTED }
        && trimmed.contains("Using \"")
        && trimmed.contains("with driver:")
    {
        if let Some(start) = trimmed.find('"')
            && let Some(end) = trimmed[start + 1..].find('"')
        {
            let device_name = &trimmed[start + 1..start + 1 + end];
            if ty == Type::General || ty == Type::Validation {
                eprintln!("\x1b[92m[Vulkan]\x1b[0m Using device: {}", device_name);
                unsafe {
                    DEVICE_REPORTED = true;
                }
            }
        }
        return vk::FALSE;
    }

    match (severity, ty) {
        (Severity::Error, _) => {
            let prefix = match ty {
                Type::Validation => "[Validation Error]",
                Type::Performance => "[Performance Error]",
                Type::General => "[General Error]",
                _ => "[Error]",
            };
            eprintln!("\x1b[91m{}\x1b[0m {}", prefix, trimmed);
        }

        (Severity::Warning, _) => {
            let prefix = match ty {
                Type::Validation => "[Validation Warning]",
                Type::Performance => "[Performance Warning]",
                Type::General => "[General Warning]",
                _ => "[Warning]",
            };
            eprintln!("\x1b[93m{}\x1b[0m {}", prefix, trimmed);
        }

        (Severity::Info, ty) => {
            if ty == Type::General {
                if trimmed.contains("vkCreateInstance")
                    || trimmed.contains("vkCreateDevice")
                    || trimmed.contains("vkCreateSwapchain")
                {
                    if trimmed.contains("success") || trimmed.contains("created") {
                        eprintln!("\x1b[96m[Info]\x1b[0m {}", trimmed);
                    }
                } else if trimmed.contains("Device")
                    || trimmed.contains("Queue")
                    || trimmed.contains("Swapchain")
                    || trimmed.contains("Memory")
                    || trimmed.contains("surface")
                    || trimmed.contains("format")
                {
                    eprintln!("\x1b[96m[Info]\x1b[0m {}", trimmed);
                }
            } else {
                let prefix = match ty {
                    Type::Validation => "[Validation]",
                    Type::Performance => "[Performance]",
                    _ => "[Info]",
                };
                eprintln!("\x1b[96m{}\x1b[0m {}", prefix, trimmed);
            }
        }

        (Severity::Verbose, _) => {
            return vk::FALSE;
        }

        _ => {}
    }

    vk::FALSE
}

impl Engine {
    pub(crate) fn create_vulkan_context(window: &dyn Window) -> VulkanContextResource {
        let dispatcher = unsafe { DynamicDispatcher::new_loaded().unwrap() };
        let entry = vk::rs::Entry::new(dispatcher, DefaultAllocator);
        let (instance, debug_utils_messenger) = Self::create_instance(
            true,
            &entry,
            &window
                .rwh_06_display_handle()
                .display_handle()
                .unwrap()
                .as_raw(),
        );

        let surface = window::rs::create_surface(
            &instance,
            &window.display_handle().unwrap().as_raw(),
            &window.window_handle().unwrap().as_raw(),
        )
        .unwrap();
        let (physical_device, device, queue_family_index, graphics_queue, transfer_queue) =
            Self::create_device(&instance, &surface);

        let mut allocator_create_info =
            AllocatorCreateInfo::new(&instance, &device, &physical_device, &dispatcher);
        allocator_create_info.flags |= AllocatorCreateFlags::bufferDeviceAddress;
        let allocator = unsafe { Allocator::new(allocator_create_info).unwrap() };

        let surface_size = window.surface_size();
        let (swapchain, surface_format) =
            Self::create_swapchain(&physical_device, &device, &surface, surface_size);

        VulkanContextResource {
            instance,
            debug_utils_messenger,
            surface,
            physical_device,
            device,
            allocator,
            graphics_queue,
            transfer_queue,
            queue_family_index,
            swapchain,
            surface_format,
        }
    }

    pub fn create_instance(
        _do_enable_validation_layers: bool,
        entry: &vk::rs::Entry,
        display_handle: &RawDisplayHandle,
    ) -> (vk::rs::Instance, Option<vk::rs::DebugUtilsMessengerEXT>) {
        const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";
        let layers: Vec<_> = entry.enumerate_instance_layer_properties().unwrap();
        let has_validation = layers
            .into_iter()
            .any(|layer| layer.get_layer_name() == VALIDATION_LAYER);
        let enabled_layers = has_validation.then_some(VALIDATION_LAYER.as_ptr());

        // enable VK_EXT_debug_utils only if the validation layer is enabled
        let mut enabled_extensions =
            Vec::from(window::enumerate_required_extensions(display_handle).unwrap());
        if has_validation {
            enabled_extensions.push(vk::EXT_DEBUG_UTILS.name);
        }

        let app_info = vk::ApplicationInfo::default()
            .application_name(Some(c"Hello Triangle"))
            .engine_name(Some(c"No Engine"))
            .api_version(vk::API_VERSION_1_4);

        let mut enabled_validation_features = Vec::new();

        enabled_validation_features.push(ValidationFeatureEnableEXT::SynchronizationValidation);
        enabled_validation_features.push(ValidationFeatureEnableEXT::BestPractices);
        //enabled_validation_features.push(ValidationFeatureEnableEXT::DebugPrintf);
        //enabled_validation_features.push(ValidationFeatureEnableEXT::GpuAssisted);

        let mut validation_features = ValidationFeaturesEXT::default()
            .enabled_validation_features(enabled_validation_features.as_slice());
        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(Some(&app_info))
            .enabled_extension(&enabled_extensions)
            .enabled_layer(enabled_layers.as_slice())
            .push_next(&mut validation_features);

        let instance = entry.create_instance(&instance_info).unwrap();

        let debug_messenger = if has_validation {
            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    flagbits!(vk::DebugUtilsMessageSeverityFlagsEXT::{Info | Warning | Error | Verbose}),
                )
                .message_type(flagbits!(vk::DebugUtilsMessageTypeFlagsEXT::{General | Validation | Performance | DeviceAddressBinding}))
                .pfn_user_callback(Some(debug_callback));
            Some(
                instance
                    .create_debug_utils_messenger_ext(&debug_info)
                    .unwrap(),
            )
        } else {
            None
        };

        (instance, debug_messenger)
    }

    pub fn create_device(
        instance: &vk::rs::Instance,
        surface: &vk::rs::SurfaceKHR,
    ) -> (
        vk::rs::PhysicalDevice,
        vk::rs::Device,
        usize,
        vk::rs::Queue,
        vk::rs::Queue,
    ) {
        let physical_devices: Vec<PhysicalDevice> = instance.enumerate_physical_devices().unwrap();

        let compute_device_score = |physical_device: &vk::rs::PhysicalDevice| {
            let properties = physical_device.get_properties();
            let is_discrete = properties.device_type == vk::PhysicalDeviceType::DiscreteGpu;
            let max_2d_dim = properties.limits.max_image_dimension2_d;

            // compute a score based on if the gpu is discrete and the maximal supported 2d image dimension
            (is_discrete as u32) * 10000 + max_2d_dim
        };

        let physical_device = physical_devices
            .into_iter()
            .max_by_key(compute_device_score)
            .unwrap();

        let (queue_family_index, _) = physical_device
            .get_queue_family_properties::<Vec<_>>()
            .into_iter()
            .enumerate()
            .find(|(queue, props)| {
                props.queue_flags.contains(vk::QueueFlags::Graphics)
                    && physical_device
                        .get_surface_support_khr(*queue as u32, *surface)
                        .is_ok_and(|supported| supported)
            })
            .unwrap();

        let features = vk::PhysicalDeviceFeatures::default().shader_int64(true);

        let required_extensions = [
            vk::KHR_SWAPCHAIN.name,
            EXT_DESCRIPTOR_BUFFER.name,
            KHR_UNIFIED_IMAGE_LAYOUTS.name,
            EXT_SHADER_OBJECT.name,
            EXT_MESH_SHADER.name,
            // KHR_SHADER_NON_SEMANTIC_INFO.name,
        ];
        let mut missing_extensions: HashSet<&CStr> =
            required_extensions.iter().map(|ext| ext.get()).collect();
        for extension_prop in physical_device
            .enumerate_device_extension_properties::<Vec<_>>(None)
            .unwrap()
        {
            missing_extensions.remove(extension_prop.get_extension_name());
        }

        if !missing_extensions.is_empty() {
            missing_extensions
                .iter()
                .enumerate()
                .for_each(|(index, missing_extension)| {
                    println!("Missing Extension {index}: {:?}", missing_extension)
                });
            panic!("Detected unsupported extentions.");
        }

        let queue_prio = [1.0f32, 0.5f32];
        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index as u32)
            .queue_priorities(&queue_prio);

        let device_info = structure_chain!(
            vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_info)
                .enabled_features(Some(&features))
                .enabled_extension(&required_extensions),
            PhysicalDeviceVulkan11Features::default().shader_draw_parameters(true),
            PhysicalDeviceVulkan12Features::default()
                .buffer_device_address(true)
                .scalar_block_layout(true)
                .storage_push_constant8(true)
                .shader_int8(true)
                .descriptor_binding_partially_bound(true)
                .descriptor_binding_variable_descriptor_count(true)
                .runtime_descriptor_array(true),
            PhysicalDeviceVulkan13Features::default()
                .synchronization2(true)
                .dynamic_rendering(true),
            PhysicalDeviceRobustness2FeaturesKHR::default().null_descriptor(true),
            PhysicalDeviceUnifiedImageLayoutsFeaturesKHR::default().unified_image_layouts(true),
            PhysicalDeviceDescriptorBufferFeaturesEXT::default().descriptor_buffer(true),
            PhysicalDeviceShaderObjectFeaturesEXT::default().shader_object(true),
            PhysicalDeviceMeshShaderFeaturesEXT::default()
                .mesh_shader(true)
                .task_shader(true),
        );

        let device = physical_device.create_device(device_info.as_ref()).unwrap();
        let graphics_queue = device.get_queue(queue_family_index as u32, 0);
        let transfer_queue = device.get_queue(queue_family_index as u32, 1);

        (
            physical_device,
            device,
            queue_family_index,
            graphics_queue,
            transfer_queue,
        )
    }

    fn create_swapchain(
        physical_device: &vk::rs::PhysicalDevice,
        device: &vk::rs::Device,
        surface: &vk::rs::SurfaceKHR,
        window_size: PhysicalSize<u32>,
    ) -> (SwapchainKHR, SurfaceFormatKHR) {
        let capabilities = physical_device
            .get_surface_capabilities_khr(*surface)
            .unwrap();

        let surface_format = physical_device
            .get_surface_formats_khr::<Vec<_>>(Some(*surface))
            .unwrap()
            .into_iter()
            .max_by_key(|fmt| match fmt {
                // we have one pair of format/color_space that we prefer
                vk::SurfaceFormatKHR {
                    format: vk::Format::B8G8R8A8Srgb,
                    color_space: vk::ColorSpaceKHR::SrgbNonlinear,
                } => 1,
                _ => 0,
            })
            .unwrap();

        // Only use FIFO for the time being
        // The Vulkan spec guarantees that if the swapchain extension is supported
        // then the FIFO present mode is too
        if !physical_device
            .get_surface_present_modes_khr::<Vec<_>>(Some(*surface))
            .unwrap()
            .contains(&vk::PresentModeKHR::Fifo)
        {
            panic!("Unsupported present mode: {:?}", vk::PresentModeKHR::Fifo);
        }

        let extent = if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            let min_ex = capabilities.min_image_extent;
            let max_ex = capabilities.max_image_extent;
            vk::Extent2D {
                width: window_size.width.clamp(min_ex.width, max_ex.width),
                height: window_size.height.clamp(min_ex.height, max_ex.height),
            }
        };

        let max_swap_count = if capabilities.max_image_count != 0 {
            capabilities.max_image_count
        } else {
            u32::MAX
        };
        let swapchain_count = (capabilities.min_image_count + 1).min(max_swap_count);

        let swapchain_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(swapchain_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::ColorAttachment | vk::ImageUsageFlags::TransferDst)
            .image_sharing_mode(vk::SharingMode::Exclusive)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::Opaque)
            .present_mode(vk::PresentModeKHR::Mailbox)
            .clipped(true);

        let swapchain = device.create_swapchain_khr(&swapchain_info).unwrap();

        (swapchain, surface_format)
    }
}
