#![allow(dead_code, unused_variables, unused_imports, redundant_semicolons, unused_macros)]

use std::{ffi::CString, io::Read, str::FromStr};
pub mod utils;
pub mod constants;
pub mod gfx_headless;
pub mod gfx_window;
pub mod create_gfx;
#[macro_use]
pub mod macros;
pub mod memory;
extern crate itertools;
extern crate strum;
use itertools::Itertools;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use utils::{cstr};
use winit::{dpi::LogicalPosition, event::ElementState};
use crate::{memory::{print_flags, split_flags, split_flags_u32}, utils::print_endianness};
use gfx_headless::*;

fn main() {
  let (gfx_headless, gfx_window, event_loop) = create_gfx::create_gfx();
  unpack!(gfx_headless, entry, instance, physical_device, device, command_pool, main_queue, main_queue_family_index);
  unpack!(gfx_window, swapchain_device, swapchain, surface, surface_instance, window, window_handle, display_handle, surface_format);

  let (image_bytes, image_width, image_height) = get_garfield_bytes();
  let extent = ash::vk::Extent3D::default()
    .width(image_width)
    .height(image_height)
    .depth(1);

  // get swapchain images
  let swapchain_images = get_swapchain_images(swapchain_device, swapchain);

  // get memory_type_index for the buffer
  let memory_kind = constants::MemoryKind::Image1;
  let memory_kind_flags = memory::get_memory_flags_raw(&memory::get_memory_flags_from_kind(memory_kind));

  // allocate the raw image
  let (raw_image, raw_image_format) = create_image(device, main_queue_family_index, &extent, &ash::vk::Format::R8G8B8A8_UNORM);
  set_object_name(instance, device, raw_image, "raw image");
  let requirements = get_image_memory_requirements(device, &raw_image);
  let memory_type_bits = requirements.memory_type_bits;
  let memory_type_index = 
    memory::get_memory_type_index_raw(instance, physical_device, memory_kind_flags, memory_type_bits)
    .expect("no suitable memory type index found");
  let memory_allocation = allocate_memory(device, memory_type_index, requirements.size);
  let offset = 0;
  bind_image_memory(device, &raw_image, &memory_allocation, offset);

  // map the memory so the CPU can consume it
  let mapped_memory = map_memory(device, &memory_allocation);

  // populate the host visible image
  let image_layout = get_image_layout(device, raw_image);
  write_bytes(mapped_memory, &image_bytes, &image_layout, &extent);

  // transition raw image to TRANSFER_SRC_OPTIMAL
  transition_image_to_new_layout(device, command_pool, &raw_image, main_queue, &ash::vk::ImageLayout::UNDEFINED, &ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL);

  // make a 'new' image so we can blit onto it
  let (image, image_format) = create_image(device, main_queue_family_index, &extent, surface_format);
  set_object_name(instance, device, image, "blit image");
  let requirements = get_image_memory_requirements(device, &image);
  let memory_type_bits = requirements.memory_type_bits;
  let memory_type_index = 
    memory::get_memory_type_index_raw(instance, physical_device, memory_kind_flags, memory_type_bits)
    .expect("no suitable memory type index found");
  let memory_allocation_2 = allocate_memory(device, memory_type_index, requirements.size);
  let offset = 0;
  bind_image_memory(device, &image, &memory_allocation, offset);

  // transition blit image to TRANSFER_DST_OPTIMAL
  transition_image_to_new_layout(device, command_pool, &image, main_queue, &ash::vk::ImageLayout::UNDEFINED, &ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL);

  // blit
  copy_image_to_surface_format(device, command_pool, main_queue, &raw_image, &image, &extent);

  // transition blit and swapchain images to formats for copy
  transition_image_to_new_layout(device, command_pool, &image, main_queue, &ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL, &ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL);

  let draw = || {
    let (next_swapchain_image, next_swapchain_image_index) = get_next_swapchain_image(device, swapchain_device, swapchain, &swapchain_images);
    println!("draw triggered. swapchain image {}", next_swapchain_image_index);
    set_object_name(instance, device, *next_swapchain_image, "swapchain image");

    transition_image_to_new_layout(device, command_pool, &next_swapchain_image, main_queue, &ash::vk::ImageLayout::UNDEFINED, &ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL);

    // copy the image to the swapchain image
    copy_image_to_swapchain_image(device, command_pool, &next_swapchain_image, main_queue, &image, &extent);
  
    // prepare swapchain image for presentation
    transition_image_to_new_layout(device, command_pool, &next_swapchain_image, main_queue, &ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL, &ash::vk::ImageLayout::PRESENT_SRC_KHR);
    
    // present the image
    present_image(swapchain_device, main_queue, swapchain, next_swapchain_image_index);
  };

  {
    use winit::{
      event::{Event, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    };
    event_loop.run(|event, window_target| {
      window_target.set_control_flow(ControlFlow::Poll);
      window.request_redraw();
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
          // do keyboard stuff
        }
        Event::Resumed => {
          window.set_visible(true);
          window.request_redraw();
        },
        Event::WindowEvent { 
          event: WindowEvent::RedrawRequested,
          ..
         } => {
          draw();
         }
        _ => {}
      }
    }).expect("event loop failed");
  }
  
  unsafe { device.device_wait_idle().expect("Failed to wait for device to become idle"); }
  unsafe { swapchain_device.destroy_swapchain(*swapchain, None); }
  unsafe { surface_instance.destroy_surface(*surface, None); }
  unsafe { device.destroy_command_pool(*command_pool, None); }
  unsafe { device.free_memory(memory_allocation, None); }
  unsafe { device.free_memory(memory_allocation_2, None); }
  unsafe { device.destroy_image(raw_image, None); }
  unsafe { device.destroy_image(image, None); }
  unsafe { device.destroy_device(None); }
  unsafe { instance.destroy_instance(None); }
  println!("Finished");
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
fn create_image(device: &ash::Device, queue_family_index: u32, extent: &ash::vk::Extent3D, image_format: &ash::vk::Format) -> (ash::vk::Image, ash::vk::Format) {
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
  let tiling = ash::vk::ImageTiling::LINEAR; // in prod, use OPTIMAL
  let queue_family_indices = [queue_family_index];
  let samples = ash::vk::SampleCountFlags::TYPE_1; // no multi-sampling
  let mip_levels = 1;
  let array_layers = 1;
  let create_info = ash::vk::ImageCreateInfo::default()
    .flags(flags) 
    .image_type(image_type)
    .initial_layout(initial_layout)
    .format(*image_format)
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
  (image, *image_format)
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

fn transition_image_to_new_layout(device: &ash::Device, command_pool: &ash::vk::CommandPool, image: &ash::vk::Image, queue: &ash::vk::Queue, old_layout: &ash::vk::ImageLayout, new_layout: &ash::vk::ImageLayout) -> () {
  let command_buffer = create_command_buffer(&device, &command_pool);
  let begin_flags = ash::vk::CommandBufferUsageFlags::default();
  let begin_create_info = ash::vk::CommandBufferBeginInfo::default()
    .flags(begin_flags);
  unsafe { 
    device
    .begin_command_buffer(command_buffer, &begin_create_info)
    .expect("failed to begin command buffer");

    let image_memory_barrier = ash::vk::ImageMemoryBarrier::default()
      .old_layout(*old_layout)
      .new_layout(*new_layout)
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
  let timeout_ms = 9999;
  let timeout_ns = timeout_ms * 1000 * 1000;
  unsafe { device.wait_for_fences(&[fence], true, timeout_ns).expect("failed to wait for fence"); }
  unsafe { device.destroy_fence(fence, None); }
  unsafe { device.free_command_buffers(*command_pool, &[command_buffer]); }
}

fn copy_image_to_surface_format(device: &ash::Device, command_pool: &ash::vk::CommandPool, queue: &ash::vk::Queue, src_image: &ash::vk::Image, dst_image: &ash::vk::Image, extent: &ash::vk::Extent3D) -> () {
  let command_buffer = create_command_buffer(&device, &command_pool);
  let begin_flags = ash::vk::CommandBufferUsageFlags::default();
  let begin_create_info = ash::vk::CommandBufferBeginInfo::default()
    .flags(begin_flags);
  unsafe { 
    device
    .begin_command_buffer(command_buffer, &begin_create_info)
    .expect("failed to begin command buffer");

    let src_image_layout = ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
    let dst_image_layout = ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL;
    let src_subresource = ash::vk::ImageSubresourceLayers::default()
      .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
      .mip_level(0)
      .base_array_layer(0)
      .layer_count(1)
    ;
    let dst_subresource = ash::vk::ImageSubresourceLayers::default()
      .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
      .mip_level(0)
      .base_array_layer(0)
      .layer_count(1)
    ;
    let region = 
      ash::vk::ImageBlit::default()
      .src_offsets([ash::vk::Offset3D::default().x(0).y(0).z(0), ash::vk::Offset3D::default().x(extent.width as i32).y(extent.height as i32).z(extent.depth as i32)])
      .dst_offsets([ash::vk::Offset3D::default().x(0).y(0).z(0), ash::vk::Offset3D::default().x(extent.width as i32).y(extent.height as i32).z(extent.depth as i32)])
      .src_subresource(src_subresource)
      .dst_subresource(dst_subresource)    
    ;
    let regions = [region];
    let filter = ash::vk::Filter::LINEAR;
    device.cmd_blit_image(command_buffer, *src_image, src_image_layout, *dst_image, dst_image_layout, &regions, filter);

    device
    .end_command_buffer(command_buffer)
    .expect("failed to end command buffer");
  };

  // submit
  let fence = submit(&device, &queue, &command_buffer);

  // await for fence
  let timeout_ms = 9999;
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
  let timeout_ms = 9999;
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

fn get_swapchain_images(swapchain_device: &ash::khr::swapchain::Device, swapchain: &ash::vk::SwapchainKHR) -> Vec<ash::vk::Image> {
  let swapchain_images = unsafe { swapchain_device.get_swapchain_images(*swapchain).expect("failed to get swapchain images") };
  swapchain_images
}

fn get_next_swapchain_image<'a>(device: &ash::Device, swapchain_device: &ash::khr::swapchain::Device, swapchain: &ash::vk::SwapchainKHR, swapchain_images: &'a Vec<ash::vk::Image>) -> (&'a ash::vk::Image, u32) {
  let timeout = 9999 * 1000 * 1000;
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
  main_queue: &ash::vk::Queue,
  swapchain: &ash::vk::SwapchainKHR,
  image_index: u32,
) -> () {
  let swapchains = [*swapchain];
  let image_indices = [image_index];
  let present_info = ash::vk::PresentInfoKHR::default()
    .swapchains(&swapchains)
    .image_indices(&image_indices);

  unsafe {
    swapchain_device.queue_present(*main_queue, &present_info).expect("Failed to present swapchain image");
  }
}

fn set_object_name<H: ash::vk::Handle>(
  instance: &ash::Instance,
  device: &ash::Device,
  object_handle: H,
  name: &str,
) -> () {
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

fn get_supported_surface_formats(physical_device: &ash::vk::PhysicalDevice, surface_instance: &ash::khr::surface::Instance, surface: &ash::vk::SurfaceKHR) -> Vec<ash::vk::SurfaceFormatKHR> {
  let formats = unsafe { surface_instance.get_physical_device_surface_formats(*physical_device, *surface).expect("failed to get supported surface formats") };
  formats
}

fn get_target_surface_format(physical_device: &ash::vk::PhysicalDevice, surface_instance: &ash::khr::surface::Instance, surface: &ash::vk::SurfaceKHR) -> ash::vk::Format {
  let formats = get_supported_surface_formats(physical_device, surface_instance, surface);
  let preferences = [ash::vk::Format::R8G8B8A8_UNORM, ash::vk::Format::B8G8R8A8_UNORM];
  let first_preference = preferences.iter().find(|preference| {
    formats.iter().any(|format| {
      format.format == **preference
    })
  }).expect("failed to find any good surface formats");
  return *first_preference;
}