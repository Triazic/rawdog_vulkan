use proc_macros::{Getters};

#[derive(Getters)]
/// collection of vulkan stuff with an effectively 'static' lifetime
pub struct GFXWindow {
  // required for window context
  pub surface: ash::vk::SurfaceKHR,
  pub surface_instance: ash::khr::surface::Instance,
  pub swapchain: ash::vk::SwapchainKHR,
  pub swapchain_device: ash::khr::swapchain::Device,
  pub display_handle: raw_window_handle::RawDisplayHandle,
  pub window_handle: raw_window_handle::RawWindowHandle,
  pub window: winit::window::Window,
}