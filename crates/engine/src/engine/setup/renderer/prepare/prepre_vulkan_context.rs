use std::{collections::HashSet, ffi::CStr, mem::ManuallyDrop, sync::Arc};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, WindowHandle};
use vulkan::{
    Device, Entry, Instance, Version,
    vk::{self, *},
};
use vulkan_vma::*;
use winit::{dpi::PhysicalSize, window::Window};

use crate::engine::Engine;

use connect_renderer::*;

unsafe extern "system" fn debug_callback(
    severity: DebugUtilsMessageSeverityFlagsEXT,
    ty: DebugUtilsMessageTypeFlagsEXT,
    data: *const DebugUtilsMessengerCallbackDataEXT,
    _: *mut std::ffi::c_void,
) -> Bool32 {
    use DebugUtilsMessageSeverityFlagsEXT as Severity;
    use DebugUtilsMessageTypeFlagsEXT as Type;

    let message = unsafe { CStr::from_ptr((*data).message).to_string_lossy() };
    let trimmed = message.trim();

    static mut IN_DEVICE_SETUP: bool = false;
    static mut DEVICE_REPORTED: bool = false;

    if trimmed.contains("vkCreateDevice layer callstack setup to:") {
        unsafe {
            IN_DEVICE_SETUP = true;
        }
        return FALSE;
    }

    if trimmed.starts_with("<Device>") {
        unsafe {
            IN_DEVICE_SETUP = false;
        }
        return FALSE;
    }

    if unsafe { IN_DEVICE_SETUP } {
        return FALSE;
    }

    if trimmed.contains("Inserted device layer") {
        return FALSE;
    }

    if !unsafe { DEVICE_REPORTED }
        && trimmed.contains("Using \"")
        && trimmed.contains("with driver:")
    {
        if let Some(start) = trimmed.find('"')
            && let Some(end) = trimmed[start + 1..].find('"')
        {
            let device_name = &trimmed[start + 1..start + 1 + end];
            if ty == Type::GENERAL || ty == Type::VALIDATION {
                eprintln!("\x1b[92m[Vulkan]\x1b[0m Using device: {}", device_name);
                unsafe {
                    DEVICE_REPORTED = true;
                }
            }
        }
        return FALSE;
    }

    match (severity, ty) {
        (Severity::ERROR, _) => {
            let prefix = match ty {
                Type::VALIDATION => "[Validation Error]",
                Type::PERFORMANCE => "[Performance Error]",
                Type::GENERAL => "[General Error]",
                _ => "[Error]",
            };
            eprintln!("\x1b[91m{}\x1b[0m {}", prefix, trimmed);
        }

        (Severity::WARNING, _) => {
            let prefix = match ty {
                Type::VALIDATION => "[Validation Warning]",
                Type::PERFORMANCE => "[Performance Warning]",
                Type::GENERAL => "[General Warning]",
                _ => "[Warning]",
            };
            eprintln!("\x1b[93m{}\x1b[0m {}", prefix, trimmed);
        }

        (Severity::INFO, ty) => {
            if ty == Type::GENERAL {
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
                    Type::VALIDATION => "[Validation]",
                    Type::PERFORMANCE => "[Performance]",
                    _ => "[Info]",
                };
                eprintln!("\x1b[96m{}\x1b[0m {}", prefix, trimmed);
            }
        }

        (Severity::VERBOSE, _) => {
            return FALSE;
        }

        _ => {}
    }

    FALSE
}

impl Engine {
    pub(crate) fn create_vulkan_context(window: &dyn Window) -> VulkanContextResource {
        let loader =
            unsafe { vulkan::loader::LibloadingLoader::new(vulkan::loader::LIBRARY).unwrap() };
        let entry = unsafe { vulkan::Entry::new(loader).unwrap() };

        let (instance, debug_utils_messenger) =
            Self::create_instance(true, &entry, &window.window_handle().unwrap());

        let surface = unsafe {
            vulkan::window::create_surface(
                &instance,
                &window.display_handle().unwrap(),
                &window.window_handle().unwrap(),
            )
            .unwrap()
        };

        let (physical_device, device, queue_family_index, graphics_queue, transfer_queue) =
            Self::create_device(&instance, &surface);

        let mut allocator_create_info = AllocatorOptions::new(&instance, &device, physical_device);
        allocator_create_info.flags |= AllocatorCreateFlags::BUFFER_DEVICE_ADDRESS;
        let allocator = unsafe { Allocator::new(&allocator_create_info).unwrap() };

        let surface_size = window.surface_size();
        let (swapchain, surface_format) =
            Self::create_swapchain(&instance, physical_device, &device, surface, surface_size);

        VulkanContextResource {
            entry: ManuallyDrop::new(entry),
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
        entry: &Entry,
        window_handle: &WindowHandle,
    ) -> (Arc<Instance>, Option<DebugUtilsMessengerEXT>) {
        const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";
        let layers: Vec<_> = unsafe { entry.enumerate_instance_layer_properties().unwrap() };
        let has_validation = layers
            .into_iter()
            .any(|layer| layer.layer_name.as_cstr() == VALIDATION_LAYER);
        let enabled_layers = has_validation.then_some(VALIDATION_LAYER.as_ptr());

        // enable VK_EXT_debug_utils only if the validation layer is enabled
        let mut enabled_extensions =
            vulkan::window::get_required_instance_extensions(window_handle)
                .iter()
                .map(|e| e.as_ptr())
                .collect::<Vec<_>>();

        if has_validation {
            enabled_extensions.push(EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
        }
        enabled_extensions.push(KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION.name.as_ptr());

        let app_info = ApplicationInfoBuilder::default()
            .application_name(b"Hello Triangle\0")
            .engine_name(b"No Engine\0")
            .api_version(Version::V1_4_0.into())
            .build();

        let mut enabled_validation_features = Vec::new();

        enabled_validation_features.push(ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION);
        enabled_validation_features.push(ValidationFeatureEnableEXT::BEST_PRACTICES);
        //enabled_validation_features.push(ValidationFeatureEnableEXT::DebugPrintf);
        //enabled_validation_features.push(ValidationFeatureEnableEXT::GpuAssisted);

        let mut validation_features = ValidationFeaturesEXTBuilder::default()
            .enabled_validation_features(enabled_validation_features.as_slice())
            .build();
        let instance_info = InstanceCreateInfoBuilder::default()
            .application_info(&app_info)
            .enabled_extension_names(&enabled_extensions)
            .enabled_layer_names(enabled_layers.as_slice())
            .push_next(&mut validation_features);

        let instance = unsafe { Arc::new(entry.create_instance(&instance_info, None).unwrap()) };

        let debug_messenger = if has_validation {
            let debug_info = DebugUtilsMessengerCreateInfoEXTBuilder::default()
                .message_severity(
                    DebugUtilsMessageSeverityFlagsEXT::INFO
                        | DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
                )
                .message_type(
                    DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                        | DebugUtilsMessageTypeFlagsEXT::DEVICE_ADDRESS_BINDING,
                )
                .user_callback(Some(debug_callback));

            Some(unsafe {
                instance
                    .create_debug_utils_messenger_ext(&debug_info, None)
                    .unwrap()
            })
        } else {
            None
        };

        (instance, debug_messenger)
    }

    pub fn create_device(
        instance: &Instance,
        surface: &SurfaceKHR,
    ) -> (PhysicalDevice, Arc<Device>, usize, Queue, Queue) {
        let physical_devices: Vec<PhysicalDevice> =
            unsafe { instance.enumerate_physical_devices().unwrap() };

        let compute_device_score = |physical_device: &PhysicalDevice| {
            let properties = unsafe { instance.get_physical_device_properties(*physical_device) };
            let is_discrete = properties.device_type == PhysicalDeviceType::DISCRETE_GPU;
            let max_2d_dim = properties.limits.max_image_dimension_2d;

            // compute a score based on if the gpu is discrete and the maximal supported 2d image dimension
            (is_discrete as u32) * 10000 + max_2d_dim
        };

        let physical_device = physical_devices
            .into_iter()
            .max_by_key(compute_device_score)
            .unwrap();

        let (queue_family_index, _) = unsafe {
            instance
                .get_physical_device_queue_family_properties(physical_device)
                .into_iter()
                .enumerate()
                .find(|(queue, props)| {
                    props.queue_flags.contains(QueueFlags::GRAPHICS)
                        && unsafe {
                            instance
                                .get_physical_device_surface_support_khr(
                                    physical_device,
                                    *queue as u32,
                                    *surface,
                                )
                                .is_ok_and(|supported| supported)
                        }
                })
                .unwrap()
        };

        let required_extensions = [
            KHR_SWAPCHAIN_EXTENSION.name.as_ptr(),
            EXT_DESCRIPTOR_BUFFER_EXTENSION.name.as_ptr(),
            KHR_UNIFIED_IMAGE_LAYOUTS_EXTENSION.name.as_ptr(),
            EXT_SHADER_OBJECT_EXTENSION.name.as_ptr(),
            EXT_MESH_SHADER_EXTENSION.name.as_ptr(),
            // KHR_SHADER_NON_SEMANTIC_INFO.name,
        ];
        let mut missing_extensions: HashSet<&CStr> = required_extensions
            .iter()
            .map(|&required_extension| unsafe { CStr::from_ptr(required_extension) })
            .collect();
        for extension_property in unsafe {
            instance
                .enumerate_device_extension_properties(physical_device, None)
                .unwrap()
        } {
            missing_extensions.remove(extension_property.extension_name.as_cstr());
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
        let queue_info = [DeviceQueueCreateInfoBuilder::default()
            .queue_family_index(queue_family_index as u32)
            .queue_priorities(&queue_prio)
            .build()];

        let mut physical_device_robustness_feature =
            PhysicalDeviceRobustness2FeaturesKHRBuilder::default().null_descriptor(true);

        let mut physical_device_unified_image_layouts_feature =
            PhysicalDeviceUnifiedImageLayoutsFeaturesKHRBuilder::default()
                .unified_image_layouts(true);

        let mut physical_device_descriptor_buffer =
            PhysicalDeviceDescriptorBufferFeaturesEXTBuilder::default().descriptor_buffer(true);

        let mut physical_device_shader_object =
            PhysicalDeviceShaderObjectFeaturesEXTBuilder::default().shader_object(true);

        let mut physical_device_mesh_shader_feature =
            PhysicalDeviceMeshShaderFeaturesEXTBuilder::default()
                .mesh_shader(true)
                .task_shader(true);

        let mut physical_device_features13 = PhysicalDeviceVulkan13FeaturesBuilder::default()
            .synchronization2(true)
            .dynamic_rendering(true);

        let mut physical_device_features12 = PhysicalDeviceVulkan12Features::builder()
            .buffer_device_address(true)
            .scalar_block_layout(true)
            .storage_push_constant8(true)
            .shader_int8(true)
            .descriptor_binding_partially_bound(true)
            .descriptor_binding_variable_descriptor_count(true)
            .runtime_descriptor_array(true);
        physical_device_features12.next = physical_device_features13.next_mut();

        let physical_device_features = PhysicalDeviceFeaturesBuilder::default().shader_int64(true);

        let device_info = DeviceCreateInfo::builder()
            .queue_create_infos(&queue_info)
            .enabled_extension_names(&required_extensions)
            .enabled_features(&physical_device_features)
            .push_next(&mut physical_device_features12)
            .push_next(&mut physical_device_features13)
            .push_next(&mut physical_device_mesh_shader_feature)
            .push_next(&mut physical_device_shader_object)
            .push_next(&mut physical_device_descriptor_buffer)
            .push_next(&mut physical_device_unified_image_layouts_feature)
            .push_next(&mut physical_device_robustness_feature);

        /*         PhysicalDeviceVulkan11Features::default().shader_draw_parameters(true);
        PhysicalDeviceVulkan12Features::default()
            .buffer_device_address(true)
            .scalar_block_layout(true)
            .storage_push_constant8(true)
            .shader_int8(true)
            .descriptor_binding_partially_bound(true)
            .descriptor_binding_variable_descriptor_count(true)
            .runtime_descriptor_array(true);
        PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true);
        PhysicalDeviceRobustness2FeaturesKHR::default().null_descriptor(true);
        PhysicalDeviceUnifiedImageLayoutsFeaturesKHR::default().unified_image_layouts(true);
        PhysicalDeviceDescriptorBufferFeaturesEXT::default().descriptor_buffer(true);
        PhysicalDeviceShaderObjectFeaturesEXT::default().shader_object(true);
        PhysicalDeviceMeshShaderFeaturesEXT::default()
            .mesh_shader(true)
            .task_shader(true); */

        let device = unsafe {
            Arc::new(
                instance
                    .create_device(physical_device, device_info.as_ref(), None)
                    .unwrap(),
            )
        };
        let graphics_queue = unsafe { device.get_device_queue(queue_family_index as u32, 0) };
        let transfer_queue = unsafe { device.get_device_queue(queue_family_index as u32, 1) };

        (
            physical_device,
            device,
            queue_family_index,
            graphics_queue,
            transfer_queue,
        )
    }

    fn create_swapchain(
        instance: &Instance,
        physical_device: PhysicalDevice,
        device: &Device,
        surface: SurfaceKHR,
        window_size: PhysicalSize<u32>,
    ) -> (SwapchainKHR, SurfaceFormatKHR) {
        let capabilities = unsafe {
            instance
                .get_physical_device_surface_capabilities_khr(physical_device, surface)
                .unwrap()
        };

        let surface_format = unsafe {
            instance
                .get_physical_device_surface_formats_khr(physical_device, surface)
                .unwrap()
                .into_iter()
                .max_by_key(|fmt| match fmt {
                    // we have one pair of format/color_space that we prefer
                    vk::SurfaceFormatKHR {
                        format: Format::B8G8R8A8_SRGB,
                        color_space: ColorSpaceKHR::SRGB_NONLINEAR,
                    } => 1,
                    _ => 0,
                })
                .unwrap()
        };

        // Only use FIFO for the time being
        // The Vulkan spec guarantees that if the swapchain extension is supported
        // then the FIFO present mode is too
        if !unsafe {
            instance
                .get_physical_device_surface_present_modes_khr(physical_device, surface)
                .unwrap()
                .contains(&PresentModeKHR::FIFO)
        } {
            panic!("Unsupported present mode: {:?}", PresentModeKHR::FIFO);
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

        let swapchain_info = vk::SwapchainCreateInfoKHRBuilder::default()
            .surface(surface)
            .min_image_count(swapchain_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::MAILBOX)
            .clipped(true)
            .build();

        let swapchain = unsafe { device.create_swapchain_khr(&swapchain_info, None).unwrap() };

        (swapchain, surface_format)
    }
}
