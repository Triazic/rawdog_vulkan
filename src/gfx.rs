use derive_new::new;

#[derive(new)]
pub struct GFX {
  pub entry: ash::Entry,
  pub instance: ash::Instance,
  pub physical_device: ash::vk::PhysicalDevice,
  pub device: ash::Device,
  pub surface: ash::vk::SurfaceKHR,
  pub surface_instance: ash::khr::surface::Instance,
  pub swapchain: ash::vk::SwapchainKHR,
  pub swapchain_device: ash::khr::swapchain::Device,
  pub command_pool: ash::vk::CommandPool,
  pub queue_family_index: u32,
  pub main_queue: ash::vk::Queue,
  pub display_handle: raw_window_handle::RawDisplayHandle,
  pub window_handle: raw_window_handle::RawWindowHandle,
}