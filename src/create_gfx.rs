use crate::{constants, get_supported_surface_formats, get_target_surface_format, gfx_headless::GFXHeadless, gfx_window::GFXWindow, memory, utils};
use std::{ffi::CString, io::Read, str::FromStr};
extern crate itertools;
extern crate strum;
use itertools::Itertools;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use crate::utils::{cstr};
use winit::{dpi::LogicalPosition, event::ElementState};
use crate::{memory::{print_flags, split_flags, split_flags_u32}, utils::print_endianness};

pub fn create_gfx() -> (GFXHeadless, GFXWindow, winit::event_loop::EventLoop<()>) {
  let (image_bytes, image_width, image_height) = get_garfield_bytes();
  let extent = 
    ash::vk::Extent3D::default()
    .width(image_width)
    .height(image_height)
    .depth(1);

  // make window
  let event_loop = winit::event_loop::EventLoop::new().expect("failed to create event loop");
  let window = winit::window::WindowBuilder::new()
    .with_inner_size(winit::dpi::LogicalSize::new(image_width, image_height))
    .with_active(true)
    .with_resizable(false)
    .with_decorations(true)
    .with_enabled_buttons(winit::window::WindowButtons::all())
    .with_transparent(false)
    .with_title("some window")
    .build(&event_loop).expect("failed to create window");
  let display_handle = window.display_handle().expect("failed to get display handle");
  let window_handle = window.window_handle().expect("failed to get window handle");

  // make entry, instance, device
  let entry = create_entry();
  let instance = create_instance(&entry, display_handle.into());
  let (physical_device, device, main_queue_family_index) = create_device(&entry, &instance, &display_handle.into(), &window_handle.into());
  let queue_index = 0; // only one queue for now

  // queue
  let main_queue = get_queue(&device, main_queue_family_index, queue_index);

  // command pool
  let command_pool = create_command_pool(&device, main_queue_family_index);

  // make a surface
  let surface_instance = create_surface_instance(&entry, &instance);
  let surface = create_surface(&entry, &instance, &display_handle.into(), &window_handle.into());

  // make swapchain
  let surface_format = get_target_surface_format(&physical_device, &surface_instance, &surface);
  let (swapchain_device, swapchain) = create_swapchain(&instance, &physical_device, &device, &surface, &surface_instance, &extent, &surface_format);

  let gfx_headless = GFXHeadless {
    entry, 
    instance, 
    physical_device, 
    device, 
    command_pool, 
    main_queue_family_index, 
    main_queue, 
  };

  let gfx_window = GFXWindow {
    surface, 
    surface_instance, 
    swapchain, 
    swapchain_device, 
    display_handle: display_handle.into(), 
    window_handle: window_handle.into(),
    window,
    surface_format
  };
  (gfx_headless, gfx_window, event_loop)
}

fn get_garfield_bytes() -> (Vec<u8>, u32, u32) {
  let img = 
    image::ImageReader::open("./assets/garfield.png")
    .expect("failed to read image")
    .decode()
    .expect("failed to decode image");
  let width = img.width();
  let height = img.height();
  let bytes = img.into_rgba8().into_raw();
  (bytes, width, height)
}

fn create_entry() -> ash::Entry {
  let entry = unsafe { ash::Entry::load().expect("Could not load Vulkan") };
  entry
}

fn create_instance(entry: &ash::Entry, display_handle: raw_window_handle::RawDisplayHandle) -> ash::Instance {
  // application info
  let application_name = cstr("My Application");
  let application_version = 1;
  let engine_name = cstr("My Engine");
  let engine_version = 1;
  
  let application_info = ash::vk::ApplicationInfo::default()
    .application_name(&application_name)
    .application_version(application_version)
    .engine_name(&engine_name)
    .engine_version(engine_version)
    .api_version(constants::API_VERSION);
  
  // check that required layers are supported
  let layers = unsafe { entry.enumerate_instance_layer_properties().expect("failed to enumerate instance layer properties") };
  let assert_layer_supported = |layer_name: &str| {
    let has = layers.iter().any(|layer| {
      let name = layer.layer_name_as_c_str().expect("could not get layer name").to_str().expect("could not convert layer name to &str");
      name == layer_name
    });
    assert!(has, "instance layer {} is not supported", layer_name);
  };
  constants::REQUIRED_INSTANCE_LAYERS.iter().for_each(|layer| assert_layer_supported(layer));

  // check that required extensions are supported
  let extensions = unsafe { entry.enumerate_instance_extension_properties(None).expect("failed to enumerate instance extension properties") };
  // extensions.iter().for_each(|extension| {
  //   let name = extension.extension_name_as_c_str().expect("could not get extension name").to_str().expect("could not convert extension name to &str");
  //   println!("instance extension {}", name);
  // });
  let assert_extension_supported = |extension_name: &str| {
    let has = extensions.iter().any(|extension| {
      let name = extension.extension_name_as_c_str().expect("could not get extension name").to_str().expect("could not convert extension name to &str");
      name == extension_name
    });
    assert!(has, "instance extension {} is not supported", extension_name);
  };
  constants::REQUIRED_INSTANCE_EXTENSIONS.iter().for_each(|extension| assert_extension_supported(extension));

  // ash window instance extensions
  let ash_window_instance_extensions = ash_window::enumerate_required_extensions(display_handle).expect("failed to enumerate ash window required extensions");
  ash_window_instance_extensions.iter().for_each(|extension| {
    let weird_extension_name = extension;
    let extension_name_as_str = utils::ptr_to_str(weird_extension_name);
    assert_extension_supported(extension_name_as_str)
  });
  let ash_window_instance_extensions_strs = ash_window_instance_extensions.iter().map(|extension| utils::ptr_to_str(extension)).collect_vec();

  // instance create info
  let flags = ash::vk::InstanceCreateFlags::empty();
  let layer_cstrs = constants::REQUIRED_INSTANCE_LAYERS.iter().map(|str| cstr(str)).collect_vec();
  let layer_ptrs: Vec<*const i8> = layer_cstrs.iter().map(|s| s.as_ptr()).collect();
  let extension_strs = 
    constants::REQUIRED_INSTANCE_EXTENSIONS.iter()
    .chain(ash_window_instance_extensions_strs.iter())
    .unique()
    .collect_vec();
    ;
  let extension_cstrs = extension_strs.iter().map(|str| cstr(str)).collect_vec();
  let extension_ptrs: Vec<*const i8> = extension_cstrs.iter().map(|s| s.as_ptr()).collect();
  let instance_create_info = ash::vk::InstanceCreateInfo::default()
    .flags(flags)
    .application_info(&application_info)
    .enabled_layer_names(&layer_ptrs)
    .enabled_extension_names(&extension_ptrs);

  // create instance
  let instance = unsafe { entry.create_instance(&instance_create_info, None).expect("Could not create Vulkan instance") };
  instance
}

fn create_device(entry: &ash::Entry, instance: &ash::Instance, display_handle: &raw_window_handle::RawDisplayHandle, window_handle: &raw_window_handle::RawWindowHandle) -> (ash::vk::PhysicalDevice, ash::Device, u32) {
  // physical device
  let physical_devices = unsafe { instance.enumerate_physical_devices().expect("failed to enumerate physical devices") };
  // assert that there is at least one physical device
  assert!(physical_devices.len() > 0, "no physical devices found");
  // find the first suitable device
  let physical_device = *physical_devices.iter().find(|&physical_device| {
    let properties = unsafe { instance.get_physical_device_properties(*physical_device) };

    // number of memory allocations
    if properties.limits.max_memory_allocation_count < 1 { return false; }

    // memory flags
    if !memory::get_if_physical_device_supports_all_memory_requirements(instance, physical_device) { return false; }

    // supported image formats
    let req_formats = [ash::vk::Format::R8G8B8A8_UNORM];
    if !req_formats.iter().all(|&req_format|
      {
        let props = unsafe {
          instance.get_physical_device_format_properties(*physical_device, req_format)
        };

        let flags = ash::vk::FormatFeatureFlags::SAMPLED_IMAGE;

        let pass_linear = props.linear_tiling_features & flags == flags;
        let pass_optimal = props.optimal_tiling_features & flags == flags;
  
        pass_linear && pass_optimal
      }
    ) { return false; }

    // check vulkan version
    let required_vulkan_version = constants::API_VERSION;
    let supported_version = properties.api_version;
    if supported_version < required_vulkan_version { return false; }

    let limits = properties.limits;
    let max_image_size = limits.max_image_dimension2_d;
    // dbg!(limits.buffer_image_granularity);
    // dbg!(limits.non_coherent_atom_size);

    let sparse_properties = properties.sparse_properties;

    let features = unsafe { instance.get_physical_device_features(*physical_device) };

    let device_memory_properties = unsafe { instance.get_physical_device_memory_properties(*physical_device) };

    let memory_types = device_memory_properties.memory_types_as_slice();

    // let physical_device_format_properties = unsafe { instance.get_physical_device_format_properties(*physical_device, format) };

    // let physical_device_image_properties = unsafe { instance.get_physical_device_image_format_properties(*physical_device, format) };

    // check for required device layers
    let layers = unsafe { instance.enumerate_device_layer_properties(*physical_device).expect("failed to enumerate device layer properties") };
    let layer_supported = |layer_name: &str| {
      layers.iter().any(|layer| {
        let name = layer.layer_name_as_c_str().expect("could not get layer name").to_str().expect("could not convert layer name to &str");
        name == layer_name
      })
    };
    let all_layers_supported = constants::REQUIRED_DEVICE_LAYERS.iter().all(|layer| layer_supported(layer));
    if !all_layers_supported { return false; }

    // check for required device extensions
    let extensions = unsafe { instance.enumerate_device_extension_properties(*physical_device).expect("failed to enumerate device extension properties") };
    let extension_supported = |extension_name: &str| {
      extensions.iter().any(|extension| {
        let name = extension.extension_name_as_c_str().expect("could not get extension name").to_str().expect("could not convert extension name to &str");
        name == extension_name
      })
    };
    let all_extensions_supported = constants::REQUIRED_DEVICE_EXTENSIONS.iter().all(|extension| extension_supported(extension));
    if !all_extensions_supported { return false; }

    true // device is adequate
  }).expect("no physical device satisifies the requirements of this application");

  let queue_family_properties = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
  // find index of first suitable queue family
  assert!(queue_family_properties.len() > 0, "no queue families found");
  let queue_family_index = queue_family_properties.iter().enumerate().position(|(i, &properties)| {
    if properties.queue_count < 1 { return false; }
    let req_flags = [
      ash::vk::QueueFlags::GRAPHICS, 
      ash::vk::QueueFlags::COMPUTE, 
      ash::vk::QueueFlags::TRANSFER,
    ];
    for req_flag in req_flags.iter() {
      if !properties.queue_flags.contains(*req_flag) { return false; }
    }

    // check for surface / presentation support
    let surface_instance = ash::khr::surface::Instance::new(entry, instance);
    let surface = create_surface(entry, instance, display_handle, window_handle);
    let surface_support = unsafe { surface_instance.get_physical_device_surface_support(physical_device, i as u32, surface).expect("failed to get physical device surface support") };
    if !surface_support { return false; }
    unsafe { surface_instance.destroy_surface(surface, None); }

    true // queue family is adequate
  }).expect("no queue family satisifies the requirements of this application");

  // queue create info
  let queue_priorities = [1.0];
  let main_queue =  ash::vk::DeviceQueueCreateInfo::default()
    .queue_family_index(queue_family_index as u32)
    .queue_priorities(&queue_priorities);

  let queue_create_infos = vec![main_queue];

  // device create info
  let extension_strs: Vec<&str> = constants::REQUIRED_DEVICE_EXTENSIONS.into_iter().collect_vec();
  let extension_cstrs = extension_strs.iter().map(|str| cstr(str)).collect_vec();
  let extension_ptrs: Vec<*const i8> = extension_cstrs.iter().map(|s| s.as_ptr()).collect();
  let device_features = ash::vk::PhysicalDeviceFeatures::default();
  let device_create_info = ash::vk::DeviceCreateInfo::default()
    .queue_create_infos(&queue_create_infos)
    .enabled_extension_names(&extension_ptrs)
    .enabled_features(&device_features);

  // create device
  let device = unsafe { instance.create_device(physical_device, &device_create_info, None).expect("Could not create Vulkan device") };
  (physical_device, device, queue_family_index as u32)
}

fn get_queue(device: &ash::Device, queue_family_index: u32, queue_index: u32) -> ash::vk::Queue { 
  let queue = unsafe { device.get_device_queue(queue_family_index, queue_index) };
  return queue;
}

fn create_command_pool(device: &ash::Device, queue_family_index: u32) -> ash::vk::CommandPool {
  let flags = ash::vk::CommandPoolCreateFlags::empty();
  let create_info = ash::vk::CommandPoolCreateInfo::default()
    .flags(flags)
    .queue_family_index(queue_family_index)
    .flags(flags);
  let command_pool = unsafe { device.create_command_pool(&create_info, None).expect("failed to create command pool") };
  return command_pool;
}

fn create_surface_instance(entry: &ash::Entry, instance: &ash::Instance) -> ash::khr::surface::Instance {
  let surface = ash::khr::surface::Instance::new(&entry, &instance);
  surface
}

fn create_surface(entry: &ash::Entry, instance: &ash::Instance, display_handle: &raw_window_handle::RawDisplayHandle, window_handle: &raw_window_handle::RawWindowHandle) -> ash::vk::SurfaceKHR {
  let surface = unsafe { ash_window::create_surface(entry, instance, *display_handle, *window_handle, None).expect("failed to create surface") };
  surface
}

fn create_swapchain(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice, device: &ash::Device, surface: &ash::vk::SurfaceKHR, surface_instance: &ash::khr::surface::Instance, extent: &ash::vk::Extent3D, surface_format: &ash::vk::Format) -> (ash::khr::swapchain::Device, ash::vk::SwapchainKHR) {
  let swapchain_device = ash::khr::swapchain::Device::new(instance, device);
  let physical_device_surface_capabilities = unsafe { surface_instance.get_physical_device_surface_capabilities(*physical_device, *surface).expect("failed to get physical device surface capabilities") };
  let max_images = physical_device_surface_capabilities.min_image_count;
  let desired_image_count = max_images;
  let supported_usage_flags = physical_device_surface_capabilities.supported_usage_flags;
  let usage_flags = ash::vk::ImageUsageFlags::COLOR_ATTACHMENT | ash::vk::ImageUsageFlags::TRANSFER_DST;
  assert!(usage_flags & supported_usage_flags == usage_flags, "at least one image usage flag is not supported");
  let color_space = ash::vk::ColorSpaceKHR::SRGB_NONLINEAR;
  let pre_transform = ash::vk::SurfaceTransformFlagsKHR::IDENTITY;
  let present_mode = ash::vk::PresentModeKHR::MAILBOX;
  let extent = ash::vk::Extent2D::default().width(extent.width).height(extent.height);
  let create_info =  
    ash::vk::SwapchainCreateInfoKHR::default()
    .surface(*surface)
    .min_image_count(desired_image_count)
    .image_color_space(color_space)
    .image_format(*surface_format)
    .image_extent(extent)
    .image_usage(usage_flags)
    .image_sharing_mode(ash::vk::SharingMode::EXCLUSIVE)
    .pre_transform(pre_transform)
    .composite_alpha(ash::vk::CompositeAlphaFlagsKHR::OPAQUE)
    .present_mode(present_mode)
    .clipped(true)
    .image_array_layers(1);
    ;
  let swapchain = unsafe { swapchain_device.create_swapchain(&create_info, None).expect("failed to create swapchain") };
  (swapchain_device, swapchain)
}