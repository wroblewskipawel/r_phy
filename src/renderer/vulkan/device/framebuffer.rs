pub mod presets;

use std::{error::Error, marker::PhantomData, usize};

use ash::vk::{self, Extent2D};

use crate::renderer::vulkan::device::{AttachmentProperties, VulkanDevice};

use super::{image::VulkanImage2D, render_pass::RenderPassConfig};

pub trait ClearValue {
    fn get(&self) -> Option<vk::ClearValue>;
}

pub struct ClearNone {}

impl ClearValue for ClearNone {
    fn get(&self) -> Option<vk::ClearValue> {
        None
    }
}

pub struct ClearColor {
    pub color: vk::ClearColorValue,
}

impl ClearValue for ClearColor {
    fn get(&self) -> Option<vk::ClearValue> {
        Some(vk::ClearValue { color: self.color })
    }
}

pub struct ClearDeptStencil {
    pub depth_stencil: vk::ClearDepthStencilValue,
}

impl ClearValue for ClearDeptStencil {
    fn get(&self) -> Option<vk::ClearValue> {
        Some(vk::ClearValue {
            depth_stencil: self.depth_stencil,
        })
    }
}

pub struct ClearValueTerminator {}

impl ClearValue for ClearValueTerminator {
    fn get(&self) -> Option<vk::ClearValue> {
        unreachable!()
    }
}

fn write_clear_values<N: ClearValueList + ?Sized>(
    node: &N,
    mut vec: Vec<Option<vk::ClearValue>>,
) -> Vec<Option<vk::ClearValue>> {
    if N::LEN > 0 {
        vec.push(node.get());
        write_clear_values(node.next(), vec)
    } else {
        vec
    }
}

pub trait ClearValueList {
    const LEN: usize;
    type Item: ClearValue;
    type Next: ClearValueList;

    fn values(&self) -> Vec<Option<vk::ClearValue>> {
        write_clear_values(self, Vec::with_capacity(Self::LEN))
    }

    fn get(&self) -> Option<vk::ClearValue>;

    fn next(&self) -> &Self::Next;
}

impl ClearValueList for ClearValueTerminator {
    const LEN: usize = 0;
    type Item = Self;
    type Next = Self;

    fn get(&self) -> Option<vk::ClearValue> {
        unreachable!()
    }

    fn next(&self) -> &Self::Next {
        unreachable!()
    }
}

pub struct ClearValueNode<C: ClearValue, N: ClearValueList> {
    value: C,
    next: N,
}

impl<C: ClearValue, N: ClearValueList> ClearValueList for ClearValueNode<C, N> {
    const LEN: usize = Self::Next::LEN + 1;
    type Item = C;
    type Next = N;

    fn get(&self) -> Option<vk::ClearValue> {
        self.value.get()
    }

    fn next(&self) -> &Self::Next {
        &self.next
    }
}

pub struct ClearValueBuilder<C: ClearValueList> {
    clear_values: C,
}

impl Default for ClearValueBuilder<ClearValueTerminator> {
    fn default() -> Self {
        Self::new()
    }
}

impl ClearValueBuilder<ClearValueTerminator> {
    pub fn new() -> Self {
        Self {
            clear_values: ClearValueTerminator {},
        }
    }
}

impl<V: ClearValueList> ClearValueBuilder<V> {
    pub fn push<N: ClearValue>(self, value: N) -> ClearValueBuilder<ClearValueNode<N, V>> {
        let Self { clear_values } = self;
        ClearValueBuilder {
            clear_values: ClearValueNode {
                value,
                next: clear_values,
            },
        }
    }

    pub fn get_clear_values(&self) -> Vec<vk::ClearValue> {
        self.clear_values.values().into_iter().flatten().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentTarget {
    Color,
    DepthStencil,
    Resolve,
    Input,
    Preserve,
}

pub struct InputAttachment {
    pub image_view: vk::ImageView,
}

impl From<VulkanImage2D> for InputAttachment {
    fn from(image: VulkanImage2D) -> Self {
        Self {
            image_view: image.image_view,
        }
    }
}

impl From<&InputAttachment> for vk::DescriptorImageInfo {
    fn from(attachment: &InputAttachment) -> Self {
        vk::DescriptorImageInfo {
            image_view: attachment.image_view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            sampler: vk::Sampler::null(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AttachmentReference {
    pub target: AttachmentTarget,
    pub layout: vk::ImageLayout,
    pub usage: vk::ImageUsageFlags,
}

pub struct IndexedAttachmentReference {
    pub reference: AttachmentReference,
    pub index: u32,
}
pub struct AttachmentReferenceTerminator {}

pub trait AttachmentReferenceList {
    const LEN: usize;
    type Next: AttachmentReferenceList;

    fn values(&self, offset: usize) -> Vec<Option<IndexedAttachmentReference>>;

    fn next(&self) -> &Self::Next;

    fn get_value(&self) -> Option<AttachmentReference>;
}

impl AttachmentReferenceList for AttachmentReferenceTerminator {
    const LEN: usize = 0;
    type Next = Self;

    fn values(&self, _offset: usize) -> Vec<Option<IndexedAttachmentReference>> {
        vec![]
    }

    fn next(&self) -> &Self::Next {
        unreachable!()
    }

    fn get_value(&self) -> Option<AttachmentReference> {
        unreachable!()
    }
}

pub struct AttachmentReferenceNode<N: AttachmentReferenceList> {
    reference: Option<AttachmentReference>,
    next: N,
}

fn write_references<N: AttachmentReferenceList + ?Sized>(
    node: &N,
    offset: usize,
    mut vec: Vec<Option<IndexedAttachmentReference>>,
) -> Vec<Option<IndexedAttachmentReference>> {
    if N::LEN > 0 {
        vec.push(
            node.get_value()
                .map(|reference| IndexedAttachmentReference {
                    reference,
                    index: (N::LEN - 1 + offset) as u32,
                }),
        );
        write_references(node.next(), offset, vec)
    } else {
        vec
    }
}

impl<N: AttachmentReferenceList> AttachmentReferenceList for AttachmentReferenceNode<N> {
    const LEN: usize = Self::Next::LEN + 1;
    type Next = N;

    fn values(&self, offset: usize) -> Vec<Option<IndexedAttachmentReference>> {
        write_references(self, offset, Vec::with_capacity(Self::LEN))
    }

    fn next(&self) -> &Self::Next {
        &self.next
    }

    fn get_value(&self) -> Option<AttachmentReference> {
        self.reference
    }
}

pub struct AttachmentReferenceBuilder<A: AttachmentList> {
    pub references: A::ReferenceListType,
}

impl Default for AttachmentReferenceBuilder<AttachmentTerminator> {
    fn default() -> Self {
        Self::new()
    }
}

impl AttachmentReferenceBuilder<AttachmentTerminator> {
    pub fn new() -> Self {
        Self {
            references: AttachmentReferenceTerminator {},
        }
    }
}

impl<A: AttachmentList> AttachmentReferenceBuilder<A> {
    pub fn push<N: Attachment>(
        self,
        reference: Option<AttachmentReference>,
    ) -> AttachmentReferenceBuilder<AttachmentNode<N, A>> {
        let Self { references } = self;
        AttachmentReferenceBuilder {
            references: AttachmentReferenceNode {
                reference,
                next: references,
            },
        }
    }
}

pub trait AttachmentReferences {
    type Attachments: AttachmentList;

    fn get_references(&self) -> Vec<Option<IndexedAttachmentReference>>;
    fn get_input_attachments(
        &self,
        framebuffer: &Framebuffer<Self::Attachments>,
    ) -> Vec<InputAttachment>;
}

impl<A: AttachmentList> AttachmentReferences for AttachmentReferenceBuilder<A> {
    type Attachments = A;

    fn get_references(&self) -> Vec<Option<IndexedAttachmentReference>> {
        AttachmentReferenceList::values(&self.references, 0)
            .into_iter()
            .rev()
            .collect()
    }

    fn get_input_attachments(
        &self,
        framebuffer: &Framebuffer<Self::Attachments>,
    ) -> Vec<InputAttachment> {
        self.get_references()
            .into_iter()
            .zip(&framebuffer.attachments)
            .filter_map(|(reference, &attachment)| {
                if let Some(reference) = reference {
                    if reference.reference.target == AttachmentTarget::Input {
                        return Some(InputAttachment {
                            image_view: attachment,
                        });
                    }
                }
                None
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AttachmentTransition {
    pub load_op: vk::AttachmentLoadOp,
    pub store_op: vk::AttachmentStoreOp,
    pub initial_layout: vk::ImageLayout,
    pub final_layout: vk::ImageLayout,
}

pub struct AttachmentTransitionTerminator {}

pub trait AttachmentTransitionList {
    const LEN: usize;
    type Next: AttachmentTransitionList;

    fn values(&self) -> Vec<AttachmentTransition>;

    fn next(&self) -> &Self::Next;

    fn get_value(&self) -> AttachmentTransition;
}

impl AttachmentTransitionList for AttachmentTransitionTerminator {
    const LEN: usize = 0;
    type Next = Self;

    fn values(&self) -> Vec<AttachmentTransition> {
        vec![]
    }

    fn next(&self) -> &Self::Next {
        unreachable!()
    }

    fn get_value(&self) -> AttachmentTransition {
        unreachable!()
    }
}

pub struct AttachmentTransitionNode<N: AttachmentTransitionList> {
    transition: AttachmentTransition,
    next: N,
}

fn write_transitions<N: AttachmentTransitionList + ?Sized>(
    node: &N,
    mut vec: Vec<AttachmentTransition>,
) -> Vec<AttachmentTransition> {
    if N::LEN > 0 {
        vec.push(node.get_value());
        write_transitions(node.next(), vec)
    } else {
        vec
    }
}

impl<N: AttachmentTransitionList> AttachmentTransitionList for AttachmentTransitionNode<N> {
    const LEN: usize = Self::Next::LEN + 1;
    type Next = N;

    fn values(&self) -> Vec<AttachmentTransition> {
        write_transitions(self, Vec::with_capacity(Self::LEN))
    }

    fn next(&self) -> &Self::Next {
        &self.next
    }

    fn get_value(&self) -> AttachmentTransition {
        self.transition
    }
}

pub struct AttachmentTransitionBuilder<A: AttachmentTransitionList> {
    transitions: A,
}

impl Default for AttachmentTransitionBuilder<AttachmentTransitionTerminator> {
    fn default() -> Self {
        Self::new()
    }
}

impl AttachmentTransitionBuilder<AttachmentTransitionTerminator> {
    pub fn new() -> Self {
        Self {
            transitions: AttachmentTransitionTerminator {},
        }
    }
}

impl<A: AttachmentTransitionList> AttachmentTransitionBuilder<A> {
    pub fn push(
        self,
        transition: AttachmentTransition,
    ) -> AttachmentTransitionBuilder<AttachmentTransitionNode<A>> {
        let Self { transitions } = self;
        AttachmentTransitionBuilder {
            transitions: AttachmentTransitionNode {
                transition,
                next: transitions,
            },
        }
    }
}

pub trait AttachmentTransistions {
    fn get(&self) -> Vec<AttachmentTransition>;
}

impl<A: AttachmentTransitionList> AttachmentTransistions for AttachmentTransitionBuilder<A> {
    fn get(&self) -> Vec<AttachmentTransition> {
        self.transitions.values().into_iter().rev().collect()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AttachmentFormatInfo {
    pub format: vk::Format,
    pub samples: vk::SampleCountFlags,
}

pub trait Attachment: 'static {
    type Clear: ClearValue;

    fn get_format(properties: &AttachmentProperties) -> AttachmentFormatInfo;
}

fn write_image_views<N: AttachmentList + ?Sized>(
    node: &N,
    mut vec: Vec<vk::ImageView>,
) -> Vec<vk::ImageView> {
    if N::LEN > 0 {
        vec.push(node.view());
        write_image_views(node.next(), vec)
    } else {
        vec
    }
}

pub trait AttachmentList: 'static {
    const LEN: usize;
    type Item: Attachment;
    type Next: AttachmentList;
    type ClearListType: ClearValueList;
    type ReferenceListType: AttachmentReferenceList;
    type TransitionListType: AttachmentTransitionList;

    fn values(&self) -> Vec<vk::ImageView> {
        write_image_views(self, Vec::with_capacity(Self::LEN))
    }

    fn next(&self) -> &Self::Next;

    fn view(&self) -> vk::ImageView;
}

fn write_formats<N: AttachmentList + ?Sized>(
    properties: &AttachmentProperties,
    mut vec: Vec<AttachmentFormatInfo>,
) -> Vec<AttachmentFormatInfo> {
    if N::LEN > 0 {
        vec.push(N::Item::get_format(properties));
        write_formats::<N::Next>(properties, vec)
    } else {
        vec
    }
}

pub trait AttachmentListFormats: AttachmentList {
    fn values(properties: &AttachmentProperties) -> Vec<AttachmentFormatInfo> {
        write_formats::<Self>(properties, Vec::with_capacity(Self::LEN))
            .into_iter()
            .rev()
            .collect()
    }
}

impl<T: AttachmentList> AttachmentListFormats for T {}

pub struct AttachmentTerminator {}

impl Attachment for AttachmentTerminator {
    type Clear = ClearNone;

    fn get_format(_properties: &AttachmentProperties) -> AttachmentFormatInfo {
        panic!("get_format called on AttachmentTerminator!");
    }
}

impl AttachmentList for AttachmentTerminator {
    const LEN: usize = 0;
    type Item = Self;
    type Next = Self;
    type ClearListType = ClearValueTerminator;
    type ReferenceListType = AttachmentReferenceTerminator;
    type TransitionListType = AttachmentTransitionTerminator;

    fn next(&self) -> &Self::Next {
        unreachable!()
    }

    fn view(&self) -> vk::ImageView {
        unreachable!()
    }
}

pub struct AttachmentNode<A: Attachment, N: AttachmentList> {
    view: vk::ImageView,
    next: N,
    _phantom: PhantomData<A>,
}

impl<A: Attachment, N: AttachmentList> AttachmentList for AttachmentNode<A, N> {
    const LEN: usize = N::LEN + 1;
    type Item = A;
    type Next = N;
    type ClearListType = ClearValueNode<A::Clear, N::ClearListType>;
    type ReferenceListType = AttachmentReferenceNode<N::ReferenceListType>;
    type TransitionListType = AttachmentTransitionNode<N::TransitionListType>;

    fn next(&self) -> &Self::Next {
        &self.next
    }

    fn view(&self) -> vk::ImageView {
        self.view
    }
}

pub struct AttachmentsBuilder<A: AttachmentList> {
    attachments: A,
}

impl Default for AttachmentsBuilder<AttachmentTerminator> {
    fn default() -> Self {
        Self::new()
    }
}

impl AttachmentsBuilder<AttachmentTerminator> {
    pub fn new() -> Self {
        Self {
            attachments: AttachmentTerminator {},
        }
    }
}

impl<A: AttachmentList> AttachmentsBuilder<A> {
    pub fn push<N: Attachment>(
        self,
        view: vk::ImageView,
    ) -> AttachmentsBuilder<AttachmentNode<N, A>> {
        let Self { attachments } = self;
        AttachmentsBuilder {
            attachments: AttachmentNode {
                view,
                next: attachments,
                _phantom: PhantomData,
            },
        }
    }

    pub fn get_attachments(&self) -> Vec<vk::ImageView> {
        self.attachments.values().into_iter().collect()
    }
}

pub type Builder<A> = AttachmentsBuilder<A>;

pub type References<A> = AttachmentReferenceBuilder<A>;

pub type Transitions<A> = AttachmentTransitionBuilder<<A as AttachmentList>::TransitionListType>;

pub type Clear<A> = ClearValueBuilder<<A as AttachmentList>::ClearListType>;

#[derive(Debug)]
pub struct Framebuffer<A: AttachmentList> {
    pub framebuffer: vk::Framebuffer,
    pub attachments: Vec<vk::ImageView>,
    _phantom: PhantomData<A>,
}

#[derive(Debug)]
pub struct FramebufferHandle<A: AttachmentList> {
    pub framebuffer: vk::Framebuffer,
    _phantom: PhantomData<A>,
}

impl<A: AttachmentList> Clone for FramebufferHandle<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: AttachmentList> From<&Framebuffer<A>> for FramebufferHandle<A> {
    fn from(framebuffer: &Framebuffer<A>) -> Self {
        Self {
            framebuffer: framebuffer.framebuffer,
            _phantom: PhantomData,
        }
    }
}

impl<A: AttachmentList> Copy for FramebufferHandle<A> {}

impl VulkanDevice {
    pub fn build_framebuffer<C: RenderPassConfig>(
        &self,
        builder: Builder<C::Attachments>,
        extent: Extent2D,
    ) -> Result<Framebuffer<C::Attachments>, Box<dyn Error>> {
        let render_pass = self.get_render_pass::<C>()?;
        let attachments = builder.get_attachments();
        let create_info = vk::FramebufferCreateInfo::builder()
            .attachments(&attachments)
            .render_pass(render_pass.handle)
            .width(extent.width)
            .height(extent.height)
            .layers(1);
        let framebuffer = unsafe { self.device.create_framebuffer(&create_info, None)? };
        Ok(Framebuffer {
            framebuffer,
            attachments,
            _phantom: PhantomData,
        })
    }

    pub fn destroy_framebuffer<A: AttachmentList>(&self, framebuffer: &mut Framebuffer<A>) {
        unsafe {
            self.device
                .destroy_framebuffer(framebuffer.framebuffer, None);
        }
    }
}
