use ash::vk;
use std::error::Error;

#[derive(Debug, Clone, Copy)]
pub struct VulkanRendererConfig {
    pub page_size: vk::DeviceSize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VulkanRendererConfigBuilder {
    page_size: Option<vk::DeviceSize>,
}

impl VulkanRendererConfig {
    pub fn builder() -> VulkanRendererConfigBuilder {
        VulkanRendererConfigBuilder::default()
    }
}

impl VulkanRendererConfigBuilder {
    pub fn build(self) -> Result<VulkanRendererConfig, Box<dyn Error>> {
        let config = VulkanRendererConfig {
            page_size: self.page_size.ok_or("Page size not provided")?,
        };
        Ok(config)
    }

    pub fn with_page_size(mut self, size: usize) -> Self {
        self.page_size = Some(size as vk::DeviceSize);
        self
    }
}
