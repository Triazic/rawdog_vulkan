#![allow(dead_code, unused_variables, unused_imports, redundant_semicolons)]

use std::{ffi::CString, io::Read, str::FromStr};
pub mod utils;
pub mod constants;
pub mod gfx;
pub mod memory;
extern crate itertools;
extern crate strum;
use itertools::Itertools;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use utils::{cstr};
use winit::{dpi::LogicalPosition, event::ElementState};
use crate::{gfx::GFX, memory::{print_flags, split_flags, split_flags_u32}, utils::print_endianness};

fn main() {
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
  let (physical_device, device, queue_family_index) = create_device(&entry, &instance, &display_handle.into(), &window_handle.into());
  let queue_index = 0; // only one queue for now

  // get memory_type_index for the buffer
  let memory_kind = constants::MemoryKind::Image1;
  let memory_kind_flags = memory::get_memory_flags_raw(&memory::get_memory_flags_from_kind(memory_kind));

  // allocate the image
  let (image, image_format) = create_image(&device, queue_family_index, &extent);
  set_object_name(&instance, &device, image, "working image");
  let requirements = get_image_memory_requirements(&device, &image);
  let memory_type_bits = requirements.memory_type_bits;
  let memory_type_index = 
    memory::get_memory_type_index_raw(&instance, &physical_device, memory_kind_flags, memory_type_bits)
    .expect("no suitable memory type index found");
  let memory_allocation = allocate_memory(&device, memory_type_index, requirements.size);
  let offset = 0;
  bind_image_memory(&device, &image, &memory_allocation, offset);

  // map the memory so the CPU can consume it
  let mapped_memory = map_memory(&device, &memory_allocation);

  // populate the host visible image
  let image_layout = get_image_layout(&device, image);
  write_bytes(mapped_memory, &image_bytes, &image_layout, &extent);
  print_image(mapped_memory, &image_layout, &extent, &image_format);

  // queue
  let main_queue = get_queue(&device, queue_family_index, queue_index);

  // command pool
  let command_pool = create_command_pool(&device, queue_family_index);

  // make a surface
  let surface_instance = create_surface_instance(&entry, &instance);
  let surface = create_surface(&entry, &instance, &display_handle.into(), &window_handle.into());

  // make swapchain
  let (swapchain_device, swapchain) = create_swapchain(&instance, &physical_device, &device, &surface, &surface_instance, &extent);

  // get swapchain images
  let swapchain_images = get_swapchain_images(&swapchain_device, &swapchain);
  let (next_swapchain_image, next_swapchain_image_index) = get_next_swapchain_image(&device, &swapchain_device, &swapchain, &swapchain_images);
  set_object_name(&instance, &device, *next_swapchain_image, "swapchain image");

  // transition images to state appropriate for copy
  transition_image_to_transfer_src_mode(&device, &command_pool, &image, &main_queue);
  transition_swapchain_image_to_transfer_dst_mode(&device, &command_pool, &next_swapchain_image, &main_queue);

  // copy the image to the swapchain image
  copy_image_to_swapchain_image(&device, &command_pool, &next_swapchain_image, &main_queue, &image, &extent);

  // prepare swapchain image for presentation
  transition_swapchain_image_to_present_mode(&device, &command_pool, &next_swapchain_image, &main_queue);

  // present the image
  present_image(&swapchain_device, &main_queue, &swapchain, next_swapchain_image_index);

  use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
  };
  event_loop.run(|event, window_target| {
    window_target.set_control_flow(ControlFlow::Poll);

    match event {
      Event::WindowEvent {
          event: WindowEvent::CloseRequested,
          ..
      } => {
        window_target.exit();
      }
      Event::WindowEvent { 
        event: WindowEvent::KeyboardInput { device_id, event, is_synthetic },
        ..
       } => {
        window.request_redraw();
       }
      _ => {}
    }
  }).expect("event loop failed");

  let gfx = GFX::new(entry, instance, physical_device, device, surface, surface_instance, swapchain, swapchain_device, command_pool, queue_family_index, main_queue, display_handle.into(), window_handle.into());
  unsafe { gfx.device.device_wait_idle().expect("Failed to wait for device to become idle"); }
  unsafe { gfx.swapchain_device.destroy_swapchain(swapchain, None); }
  unsafe { gfx.surface_instance.destroy_surface(surface, None); }
  unsafe { gfx.device.destroy_command_pool(command_pool, None); }
  unsafe { gfx.device.free_memory(memory_allocation, None); }
  unsafe { gfx.device.destroy_image(image, None); }
  unsafe { gfx.device.destroy_device(None); }
  unsafe { gfx.instance.destroy_instance(None); }
  println!("Finished");
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

/// just a handle. not backed with memory
fn create_buffer(device: &ash::Device, buffer_size: u64) -> ash::vk::Buffer {
  let flags = ash::vk::BufferCreateFlags::empty();
  let usage = ash::vk::BufferUsageFlags::TRANSFER_SRC | ash::vk::BufferUsageFlags::TRANSFER_DST;
  let sharing_mode = ash::vk::SharingMode::EXCLUSIVE; // used in one queue
  let create_info = ash::vk::BufferCreateInfo::default()
    .flags(flags) 
    .size(buffer_size)
    .usage(usage)
    .sharing_mode(sharing_mode);

  let buffer = unsafe { device.create_buffer(&create_info, None).expect("Could not create Vulkan buffer") };
  buffer
}

/// just a handle. not backed with memory
fn create_image(device: &ash::Device, queue_family_index: u32, extent: &ash::vk::Extent3D) -> (ash::vk::Image, ash::vk::Format) {
  let flags = ash::vk::ImageCreateFlags::empty();
  let usage = 
    ash::vk::ImageUsageFlags::TRANSFER_SRC
    | ash::vk::ImageUsageFlags::TRANSFER_DST
    | ash::vk::ImageUsageFlags::SAMPLED // means that the image can be sampled from in a shader
    | ash::vk::ImageUsageFlags::COLOR_ATTACHMENT
    ;
  let sharing_mode = ash::vk::SharingMode::EXCLUSIVE; // used in one queue
  let image_type = ash::vk::ImageType::TYPE_2D;
  let initial_layout = ash::vk::ImageLayout::UNDEFINED;
  let image_format = ash::vk::Format::R8G8B8A8_UNORM;
  let tiling = ash::vk::ImageTiling::LINEAR; // in prod, use OPTIMAL
  let queue_family_indices = [queue_family_index];
  let samples = ash::vk::SampleCountFlags::TYPE_1; // no multi-sampling
  let mip_levels = 1;
  let array_layers = 1;
  let create_info = ash::vk::ImageCreateInfo::default()
    .flags(flags) 
    .image_type(image_type)
    .initial_layout(initial_layout)
    .format(image_format)
    .extent(*extent)
    .tiling(tiling)
    .usage(usage)
    .sharing_mode(sharing_mode)
    .queue_family_indices(&queue_family_indices)
    .samples(samples)
    .mip_levels(mip_levels)
    .array_layers(array_layers)
    ;

  let image = unsafe { device.create_image(&create_info, None).expect("Could not create Vulkan image") };
  (image, image_format)
}

fn allocate_memory(device: &ash::Device, memory_type_index: u32, size: u64) -> ash::vk::DeviceMemory {
  let info = ash::vk::MemoryAllocateInfo::default()
    .allocation_size(size)
    .memory_type_index(memory_type_index);

  let memory = unsafe { device.allocate_memory(&info, None).expect("Could not allocate Vulkan memory") };
  memory
}

/// useless, only works with VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT which is basically never supported. (35.06%)
fn print_memory_commitment(device: &ash::Device, allocation: &ash::vk::DeviceMemory) -> () {
  let memory_commitment = unsafe { device.get_device_memory_commitment(*allocation) };
  dbg!(memory_commitment);
}

fn get_heap_usage(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice) -> [u64; ash::vk::MAX_MEMORY_HEAPS] {
  let mut memory_budget_props = ash::vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
  
  let mut memory_props2 = ash::vk::PhysicalDeviceMemoryProperties2::default()
    .push_next(&mut memory_budget_props);
  
  unsafe { instance.get_physical_device_memory_properties2(*physical_device, &mut memory_props2); }
  
  let heap_usage = memory_budget_props.heap_usage;
  return heap_usage;
}

fn print_heap_usage(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice) -> () {
  let heap_usage = get_heap_usage(instance, physical_device);
  dbg!(heap_usage);
}

fn get_buffer_memory_requirements(device: &ash::Device, buffer: &ash::vk::Buffer) -> ash::vk::MemoryRequirements {
  let reqs = unsafe { device.get_buffer_memory_requirements(*buffer) };
  return reqs;
}

fn get_image_memory_requirements(device: &ash::Device, image: &ash::vk::Image) -> ash::vk::MemoryRequirements {
  let reqs = unsafe { device.get_image_memory_requirements(*image) };
  return reqs;
}

fn bind_buffer_memory(device: &ash::Device, buffer: &ash::vk::Buffer, memory_allocation: &ash::vk::DeviceMemory, offset: u64) -> () {
  unsafe { device.bind_buffer_memory(*buffer, *memory_allocation, offset).expect("failed to bind buffer memory") }
}

fn bind_image_memory(device: &ash::Device, image: &ash::vk::Image, memory_allocation: &ash::vk::DeviceMemory, offset: u64) -> () {
  unsafe { device.bind_image_memory(*image, *memory_allocation, offset).expect("failed to bind image memory") }
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

fn create_command_buffer(device: &ash::Device, command_pool: &ash::vk::CommandPool) -> ash::vk::CommandBuffer {
  let command_buffer_allocate_info = ash::vk::CommandBufferAllocateInfo::default()
    .command_buffer_count(1)
    .command_pool(*command_pool)
    .level(ash::vk::CommandBufferLevel::PRIMARY);
  let command_buffers = unsafe { device.allocate_command_buffers(&command_buffer_allocate_info).expect("failed to allocate command buffer") };
  return *command_buffers.get(0).expect("no command buffers created?");
}

fn record_command_buffer_buffer(device: &ash::Device, command_buffer: &ash::vk::CommandBuffer, buffer: &ash::vk::Buffer, buffer_size: u64) -> () {
  let begin_flags = ash::vk::CommandBufferUsageFlags::default();
  let begin_create_info = ash::vk::CommandBufferBeginInfo::default()
    .flags(begin_flags);
  unsafe { 
    device
    .begin_command_buffer(*command_buffer, &begin_create_info)
    .expect("failed to begin command buffer");

    let offset = 0;
    let data = 257;
    device.cmd_fill_buffer(*command_buffer, *buffer, offset, ash::vk::WHOLE_SIZE, data);

    device
    .end_command_buffer(*command_buffer)
    .expect("failed to end command buffer");
  };
}

fn transition_swapchain_image_to_present_mode(device: &ash::Device, command_pool: &ash::vk::CommandPool, swapchain_image: &ash::vk::Image, queue: &ash::vk::Queue) -> () {
  let command_buffer = create_command_buffer(&device, &command_pool);
  let begin_flags = ash::vk::CommandBufferUsageFlags::default();
  let begin_create_info = ash::vk::CommandBufferBeginInfo::default()
    .flags(begin_flags);
  unsafe { 
    device
    .begin_command_buffer(command_buffer, &begin_create_info)
    .expect("failed to begin command buffer");

    let image_memory_barrier = ash::vk::ImageMemoryBarrier::default()
      .old_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
      .new_layout(ash::vk::ImageLayout::PRESENT_SRC_KHR)
      .src_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
      .dst_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
      .image(*swapchain_image)
      .subresource_range(ash::vk::ImageSubresourceRange::default()
        .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
    );

    device.cmd_pipeline_barrier(
      command_buffer, 
      ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, 
      ash::vk::PipelineStageFlags::BOTTOM_OF_PIPE, 
      ash::vk::DependencyFlags::empty(), 
      &[], 
      &[], 
      &[image_memory_barrier]
    );

    device
    .end_command_buffer(command_buffer)
    .expect("failed to end command buffer");
  };

  // submit
  let fence = submit(&device, &queue, &command_buffer);

  // await for fence
  let timeout_ms = 16;
  let timeout_ns = timeout_ms * 1000 * 1000;
  unsafe { device.wait_for_fences(&[fence], true, timeout_ns).expect("failed to wait for fence"); }
  unsafe { device.destroy_fence(fence, None); }
  unsafe { device.free_command_buffers(*command_pool, &[command_buffer]); }
}

fn transition_swapchain_image_to_transfer_dst_mode(device: &ash::Device, command_pool: &ash::vk::CommandPool, swapchain_image: &ash::vk::Image, queue: &ash::vk::Queue) -> () {
  let command_buffer = create_command_buffer(&device, &command_pool);
  let begin_flags = ash::vk::CommandBufferUsageFlags::default();
  let begin_create_info = ash::vk::CommandBufferBeginInfo::default()
    .flags(begin_flags);
  unsafe { 
    device
    .begin_command_buffer(command_buffer, &begin_create_info)
    .expect("failed to begin command buffer");

    let image_memory_barrier = ash::vk::ImageMemoryBarrier::default()
      .old_layout(ash::vk::ImageLayout::UNDEFINED)
      .new_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
      .src_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
      .dst_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
      .image(*swapchain_image)
      .subresource_range(ash::vk::ImageSubresourceRange::default()
        .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
    );

    device.cmd_pipeline_barrier(
      command_buffer, 
      ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, 
      ash::vk::PipelineStageFlags::BOTTOM_OF_PIPE, 
      ash::vk::DependencyFlags::empty(), 
      &[], 
      &[], 
      &[image_memory_barrier]
    );

    device
    .end_command_buffer(command_buffer)
    .expect("failed to end command buffer");
  };

  // submit
  let fence = submit(&device, &queue, &command_buffer);

  // await for fence
  let timeout_ms = 16;
  let timeout_ns = timeout_ms * 1000 * 1000;
  unsafe { device.wait_for_fences(&[fence], true, timeout_ns).expect("failed to wait for fence"); }
  unsafe { device.destroy_fence(fence, None); }
  unsafe { device.free_command_buffers(*command_pool, &[command_buffer]); }
}

fn transition_image_to_transfer_src_mode(device: &ash::Device, command_pool: &ash::vk::CommandPool, image: &ash::vk::Image, queue: &ash::vk::Queue) -> () {
  let command_buffer = create_command_buffer(&device, &command_pool);
  let begin_flags = ash::vk::CommandBufferUsageFlags::default();
  let begin_create_info = ash::vk::CommandBufferBeginInfo::default()
    .flags(begin_flags);
  unsafe { 
    device
    .begin_command_buffer(command_buffer, &begin_create_info)
    .expect("failed to begin command buffer");

    let image_memory_barrier = ash::vk::ImageMemoryBarrier::default()
      .old_layout(ash::vk::ImageLayout::UNDEFINED)
      .new_layout(ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
      .src_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
      .dst_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
      .image(*image)
      .subresource_range(ash::vk::ImageSubresourceRange::default()
        .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
    );

    device.cmd_pipeline_barrier(
      command_buffer, 
      ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, 
      ash::vk::PipelineStageFlags::BOTTOM_OF_PIPE, 
      ash::vk::DependencyFlags::empty(), 
      &[], 
      &[], 
      &[image_memory_barrier]
    );

    device
    .end_command_buffer(command_buffer)
    .expect("failed to end command buffer");
  };

  // submit
  let fence = submit(&device, &queue, &command_buffer);

  // await for fence
  let timeout_ms = 16;
  let timeout_ns = timeout_ms * 1000 * 1000;
  unsafe { device.wait_for_fences(&[fence], true, timeout_ns).expect("failed to wait for fence"); }
  unsafe { device.destroy_fence(fence, None); }
  unsafe { device.free_command_buffers(*command_pool, &[command_buffer]); }
}

fn copy_image_to_swapchain_image(device: &ash::Device, command_pool: &ash::vk::CommandPool, swapchain_image: &ash::vk::Image, queue: &ash::vk::Queue, image: &ash::vk::Image, extent: &ash::vk::Extent3D) -> () {
  let command_buffer = create_command_buffer(&device, &command_pool);
  let begin_flags = ash::vk::CommandBufferUsageFlags::default();
  let begin_create_info = ash::vk::CommandBufferBeginInfo::default()
    .flags(begin_flags);
  unsafe { 
    device
    .begin_command_buffer(command_buffer, &begin_create_info)
    .expect("failed to begin command buffer");

    let src_image = image;
    let dst_image = swapchain_image;
    let src_image_layout = ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
    let dst_image_layout = ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL;
    let region = ash::vk::ImageCopy::default()
      .src_subresource(ash::vk::ImageSubresourceLayers::default()
        .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
        .mip_level(0)
        .base_array_layer(0)
        .layer_count(1)
      )
      .dst_subresource(ash::vk::ImageSubresourceLayers::default()
        .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
        .mip_level(0)
        .base_array_layer(0)
        .layer_count(1)
      )
      .extent(ash::vk::Extent3D::default()
        .width(extent.width)
        .height(extent.height)
        .depth(1)
      );
    let regions = [region];

    device.cmd_copy_image(command_buffer, *src_image, src_image_layout, *dst_image, dst_image_layout, &regions);

    device
    .end_command_buffer(command_buffer)
    .expect("failed to end command buffer");
  };

  // submit
  let fence = submit(&device, &queue, &command_buffer);

  // await for fence
  let timeout_ms = 16;
  let timeout_ns = timeout_ms * 1000 * 1000;
  unsafe { device.wait_for_fences(&[fence], true, timeout_ns).expect("failed to wait for fence"); }
  unsafe { device.destroy_fence(fence, None); }
  unsafe { device.free_command_buffers(*command_pool, &[command_buffer]); }
}

fn submit(device: &ash::Device, queue: &ash::vk::Queue, command_buffer: &ash::vk::CommandBuffer) -> ash::vk::Fence {
  let command_buffers = [*command_buffer];
  let submit_info = ash::vk::SubmitInfo::default()
    .command_buffers(&command_buffers);

  let fence = unsafe { device.create_fence(&ash::vk::FenceCreateInfo::default(), None).expect("failed to create fence") };

  unsafe { device.queue_submit(*queue, &[submit_info], fence).expect("failed to submit to queue"); }

  fence
}

fn map_memory(device: &ash::Device, memory_allocation: &ash::vk::DeviceMemory) -> *mut std::ffi::c_void {
  let flags = ash::vk::MemoryMapFlags::default();
  let pointer = unsafe { device.map_memory(*memory_allocation, 0, ash::vk::WHOLE_SIZE, flags).expect("failed to map memory") };
  pointer
}

fn print_buffer(mapped_memory: *mut std::ffi::c_void, buffer_size: u64) -> () {
  unsafe {
    // Cast the void pointer to a u8 pointer
    let byte_ptr = mapped_memory as *mut u8;

    // Create a slice from the raw pointer
    let slice = std::slice::from_raw_parts(byte_ptr, buffer_size as usize);

    // Now you can use the slice safely
    dbg!(&slice, slice.len());
  }
}

fn write_bytes(mapped_memory: *mut std::ffi::c_void, bytes: &[u8], layout: &ash::vk::SubresourceLayout, extent: &ash::vk::Extent3D) {
  unsafe {
    let byte_ptr = mapped_memory as *mut u8;
    let row_pitch = layout.row_pitch as u32;
    let image_size = row_pitch * extent.height * extent.depth * 4;
    let slice = std::slice::from_raw_parts_mut(byte_ptr, image_size as usize);

    let offset = layout.offset;
    assert!(offset == 0, "offset should be 0");

    for row in 0..extent.height {
      let x = (row * row_pitch) as usize; // base index into destination
      let y = (row * extent.width * 4) as usize; // base index into source
      let scanline_width = (extent.width * 4) as usize;
      let dst_slc = &mut slice[x..(x + scanline_width)];
      let src_slc = &bytes[y..(y + scanline_width)];
      dst_slc.copy_from_slice(src_slc);
    }
  }
}

fn print_image(
  mapped_memory: *mut std::ffi::c_void,
  layout: &ash::vk::SubresourceLayout, 
  extent: &ash::vk::Extent3D,
  format: &ash::vk::Format,
) {
  unsafe {
    let byte_ptr = mapped_memory as *const u8;

    let row_pitch = layout.row_pitch as u32;
    let image_size = row_pitch * extent.height * extent.depth * 4;
    let slice = std::slice::from_raw_parts(byte_ptr, image_size as usize);
    
    for row in 0..extent.height {
      let x = row * row_pitch; // base index into destination
      let row_slice = &slice[x as usize..(x + extent.width * 4) as usize];
      println!("{:?}", row_slice);
    }
  }
}

fn get_image_layout(device: &ash::Device, image: ash::vk::Image) -> ash::vk::SubresourceLayout {
  let subresource = ash::vk::ImageSubresource::default()
    .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
    .mip_level(0)
    .array_layer(0);
  let image_subresource_layout = unsafe { device.get_image_subresource_layout(image, subresource) };
  image_subresource_layout
}

fn read_buffer_to_cpu(mapped_memory: *mut std::ffi::c_void, buffer_size: u64) -> Vec<u32> {
  unsafe {
    // Cast the void pointer to a u8 pointer
    let byte_ptr = mapped_memory as *mut u8;

    // Create a slice from the raw pointer
    let slice = std::slice::from_raw_parts(byte_ptr, buffer_size as usize);

    let mut acc = Vec::new();
    let mut working_value = 0;
    for i in 0..slice.len() {
      let imod4 = (i % 4) as u32;
      let component = slice[i] as u32;
      let value = component * 2u32.pow(imod4);
      working_value += value;
      if imod4 == 3 {
        acc.push(working_value);
        working_value = 0;
      }
    }
    acc
  }
}

fn get_rgbw_bytes() -> Vec<u8> {
  let img = 
    image::ImageReader::open("./assets/RGBW.png")
    .expect("failed to read image")
    .decode()
    .expect("failed to decode image");
  let bytes = img.into_rgba8().into_raw();
  bytes
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

fn create_surface_instance(entry: &ash::Entry, instance: &ash::Instance) -> ash::khr::surface::Instance {
  let surface = ash::khr::surface::Instance::new(&entry, &instance);
  surface
}

fn create_surface(entry: &ash::Entry, instance: &ash::Instance, display_handle: &raw_window_handle::RawDisplayHandle, window_handle: &raw_window_handle::RawWindowHandle) -> ash::vk::SurfaceKHR {
  let surface = unsafe { ash_window::create_surface(entry, instance, *display_handle, *window_handle, None).expect("failed to create surface") };
  surface
}

fn create_swapchain(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice, device: &ash::Device, surface: &ash::vk::SurfaceKHR, surface_instance: &ash::khr::surface::Instance, extent: &ash::vk::Extent3D) -> (ash::khr::swapchain::Device, ash::vk::SwapchainKHR) {
  let swapchain_device = ash::khr::swapchain::Device::new(instance, device);
  let physical_device_surface_capabilities = unsafe { surface_instance.get_physical_device_surface_capabilities(*physical_device, *surface).expect("failed to get physical device surface capabilities") };
  let max_images = physical_device_surface_capabilities.min_image_count;
  let desired_image_count = max_images;
  let supported_usage_flags = physical_device_surface_capabilities.supported_usage_flags;
  let usage_flags = ash::vk::ImageUsageFlags::COLOR_ATTACHMENT | ash::vk::ImageUsageFlags::TRANSFER_DST;
  assert!(usage_flags & supported_usage_flags == usage_flags, "at least one image usage flag is not supported");
  let color_space = ash::vk::ColorSpaceKHR::SRGB_NONLINEAR;
  let image_format = ash::vk::Format::R8G8B8A8_UNORM;
  let pre_transform = ash::vk::SurfaceTransformFlagsKHR::IDENTITY;
  let present_mode = ash::vk::PresentModeKHR::MAILBOX;
  let extent = ash::vk::Extent2D::default().width(extent.width).height(extent.height);
  let create_info =  
    ash::vk::SwapchainCreateInfoKHR::default()
    .surface(*surface)
    .min_image_count(desired_image_count)
    .image_color_space(color_space)
    .image_format(image_format)
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

fn get_swapchain_images(swapchain_device: &ash::khr::swapchain::Device, swapchain: &ash::vk::SwapchainKHR) -> Vec<ash::vk::Image> {
  let swapchain_images = unsafe { swapchain_device.get_swapchain_images(*swapchain).expect("failed to get swapchain images") };
  swapchain_images
}

fn get_next_swapchain_image<'a>(device: &ash::Device, swapchain_device: &ash::khr::swapchain::Device, swapchain: &ash::vk::SwapchainKHR, swapchain_images: &'a Vec<ash::vk::Image>) -> (&'a ash::vk::Image, u32) {
  let timeout = 16 * 1000 * 1000;
  let semaphore = ash::vk::Semaphore::null();
  let fence = unsafe { device.create_fence(&ash::vk::FenceCreateInfo::default(), None).expect("failed to create fence") };
  let (image_index, suboptimal) = unsafe { swapchain_device.acquire_next_image(*swapchain, timeout, semaphore, fence).expect("failed to acquire next image") };
  unsafe { device.wait_for_fences(&[fence], true, timeout).expect("failed to wait for swapchain image fence"); }
  unsafe { device.destroy_fence(fence, None); }
  let image = swapchain_images.get(image_index as usize).expect("failed to get swapchain image from index");
  (image, image_index)
}

fn present_image(
  swapchain_device: &ash::khr::swapchain::Device,
  present_queue: &ash::vk::Queue,
  swapchain: &ash::vk::SwapchainKHR,
  image_index: u32,
) -> () {
  let swapchains = [*swapchain];
  let image_indices = [image_index];
  let present_info = ash::vk::PresentInfoKHR::default()
    .swapchains(&swapchains)
    .image_indices(&image_indices);

  unsafe {
    swapchain_device.queue_present(*present_queue, &present_info).expect("Failed to present swapchain image");
  }
}

fn set_object_name<T: ash::vk::Handle>(
  instance: &ash::Instance,
  device: &ash::Device,
  object_handle: T,
  name: &str,
) {
  use ash::vk;
  use std::ffi::CString;

  let debug_utils_loader = ash::ext::debug_utils::Device::new(&instance, &device);
  let name_cstr = CString::new(name).unwrap();
  let name_info = ash::vk::DebugUtilsObjectNameInfoEXT::default()
    .object_handle(object_handle)
    .object_name(&name_cstr)
    ;
  unsafe {
    debug_utils_loader
      .set_debug_utils_object_name(&name_info)
      .expect("Failed to set Vulkan object name");
  }
}