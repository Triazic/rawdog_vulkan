use derive_new::new;

#[derive(new)]
/// collection of vulkan stuff with an effectively 'static' lifetime
pub struct GFX {
  // required for all contexts
  pub entry: ash::Entry,
  pub instance: ash::Instance,
  pub physical_device: ash::vk::PhysicalDevice,
  pub device: ash::Device,
  pub command_pool: ash::vk::CommandPool,
  pub queue_family_index: u32,
  pub main_queue: ash::vk::Queue,

  // required for window context
  pub surface: ash::vk::SurfaceKHR,
  pub surface_instance: ash::khr::surface::Instance,
  pub swapchain: ash::vk::SwapchainKHR,
  pub swapchain_device: ash::khr::swapchain::Device,
  pub display_handle: raw_window_handle::RawDisplayHandle,
  pub window_handle: raw_window_handle::RawWindowHandle,
  pub event_loop: winit::event_loop::EventLoop<()>,
  pub window: winit::window::Window,
}