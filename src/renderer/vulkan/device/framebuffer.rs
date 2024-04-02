pub mod presets;

use std::{error::Error, marker::PhantomData, usize};

use ash::vk::{self, Extent2D};

use crate::renderer::vulkan::device::{AttachmentProperties, VulkanDevice};

use super::render_pass::RenderPassConfig;

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

pub struct ClearValueBuilder<C: ClearValueList, D: ClearValueList> {
    color: C,
    depth_stencil: D,
}

impl ClearValueBuilder<ClearValueTerminator, ClearValueTerminator> {
    pub fn new() -> Self {
        Self {
            color: ClearValueTerminator {},
            depth_stencil: ClearValueTerminator {},
        }
    }
}

impl<C: ClearValueList, D: ClearValueList> ClearValueBuilder<C, D> {
    pub fn push_color<N: ClearValue>(self, value: N) -> ClearValueBuilder<ClearValueNode<N, C>, D> {
        let Self {
            color,
            depth_stencil,
        } = self;
        ClearValueBuilder {
            color: ClearValueNode { value, next: color },
            depth_stencil,
        }
    }

    pub fn push_depth_stencil<N: ClearValue>(
        self,
        value: N,
    ) -> ClearValueBuilder<C, ClearValueNode<N, D>> {
        let Self {
            color,
            depth_stencil,
        } = self;
        ClearValueBuilder {
            color,
            depth_stencil: ClearValueNode {
                value,
                next: depth_stencil,
            },
        }
    }

    pub fn get_clear_values(&self) -> Vec<vk::ClearValue> {
        self.depth_stencil
            .values()
            .into_iter()
            .chain(self.color.values())
            .flatten()
            .rev()
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AttachmentTarget {
    Color,
    DepthStencil,
    Resolve,
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

pub struct AttachmentReferenceBuilder<
    C: AttachmentReferenceList,
    D: AttachmentReferenceList,
    R: AttachmentReferenceList,
> {
    pub color: C,
    pub depth_stencil: D,
    pub resolve: R,
}

impl
    AttachmentReferenceBuilder<
        AttachmentReferenceTerminator,
        AttachmentReferenceTerminator,
        AttachmentReferenceTerminator,
    >
{
    pub fn new() -> Self {
        Self {
            color: AttachmentReferenceTerminator {},
            depth_stencil: AttachmentReferenceTerminator {},
            resolve: AttachmentReferenceTerminator {},
        }
    }
}

impl<C: AttachmentReferenceList, D: AttachmentReferenceList, R: AttachmentReferenceList>
    AttachmentReferenceBuilder<C, D, R>
{
    pub fn push_color(
        self,
        reference: Option<AttachmentReference>,
    ) -> AttachmentReferenceBuilder<AttachmentReferenceNode<C>, D, R> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentReferenceBuilder {
            color: AttachmentReferenceNode {
                reference,
                next: color,
            },
            depth_stencil,
            resolve,
        }
    }

    pub fn push_depth_stencil(
        self,
        reference: Option<AttachmentReference>,
    ) -> AttachmentReferenceBuilder<C, AttachmentReferenceNode<D>, R> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentReferenceBuilder {
            color,
            depth_stencil: AttachmentReferenceNode {
                reference,
                next: depth_stencil,
            },
            resolve,
        }
    }

    pub fn push_resolve(
        self,
        reference: Option<AttachmentReference>,
    ) -> AttachmentReferenceBuilder<C, D, AttachmentReferenceNode<R>> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentReferenceBuilder {
            color,
            depth_stencil,
            resolve: AttachmentReferenceNode {
                reference,
                next: resolve,
            },
        }
    }
}

pub trait AttachmentReferences {
    fn color(&self) -> Vec<Option<IndexedAttachmentReference>>;
    fn depth_stencil(&self) -> Vec<Option<IndexedAttachmentReference>>;
    fn resolve(&self) -> Vec<Option<IndexedAttachmentReference>>;
}

impl<C: AttachmentReferenceList, D: AttachmentReferenceList, R: AttachmentReferenceList>
    AttachmentReferences for AttachmentReferenceBuilder<C, D, R>
{
    fn color(&self) -> Vec<Option<IndexedAttachmentReference>> {
        self.color.values(0)
    }

    fn depth_stencil(&self) -> Vec<Option<IndexedAttachmentReference>> {
        self.depth_stencil.values(C::LEN)
    }

    fn resolve(&self) -> Vec<Option<IndexedAttachmentReference>> {
        self.resolve.values(C::LEN + D::LEN)
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

pub struct AttachmentTransitionBuilder<
    C: AttachmentTransitionList,
    D: AttachmentTransitionList,
    R: AttachmentTransitionList,
> {
    color: C,
    depth_stencil: D,
    resolve: R,
}

impl
    AttachmentTransitionBuilder<
        AttachmentTransitionTerminator,
        AttachmentTransitionTerminator,
        AttachmentTransitionTerminator,
    >
{
    pub fn new() -> Self {
        Self {
            color: AttachmentTransitionTerminator {},
            depth_stencil: AttachmentTransitionTerminator {},
            resolve: AttachmentTransitionTerminator {},
        }
    }
}

impl<C: AttachmentTransitionList, D: AttachmentTransitionList, R: AttachmentTransitionList>
    AttachmentTransitionBuilder<C, D, R>
{
    pub fn push_color(
        self,
        transition: AttachmentTransition,
    ) -> AttachmentTransitionBuilder<AttachmentTransitionNode<C>, D, R> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentTransitionBuilder {
            color: AttachmentTransitionNode {
                transition,
                next: color,
            },
            depth_stencil,
            resolve,
        }
    }

    pub fn push_depth_stencil(
        self,
        transition: AttachmentTransition,
    ) -> AttachmentTransitionBuilder<C, AttachmentTransitionNode<D>, R> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentTransitionBuilder {
            color,
            depth_stencil: AttachmentTransitionNode {
                transition,
                next: depth_stencil,
            },
            resolve,
        }
    }

    pub fn push_resolve(
        self,
        transition: AttachmentTransition,
    ) -> AttachmentTransitionBuilder<C, D, AttachmentTransitionNode<R>> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentTransitionBuilder {
            color,
            depth_stencil,
            resolve: AttachmentTransitionNode {
                transition,
                next: resolve,
            },
        }
    }
}

pub trait AttachmentTransistions {
    fn color(&self) -> Vec<AttachmentTransition>;
    fn depth_stencil(&self) -> Vec<AttachmentTransition>;
    fn resolve(&self) -> Vec<AttachmentTransition>;
}

impl<C: AttachmentTransitionList, D: AttachmentTransitionList, R: AttachmentTransitionList>
    AttachmentTransistions for AttachmentTransitionBuilder<C, D, R>
{
    fn color(&self) -> Vec<AttachmentTransition> {
        self.color.values()
    }

    fn depth_stencil(&self) -> Vec<AttachmentTransition> {
        self.depth_stencil.values()
    }

    fn resolve(&self) -> Vec<AttachmentTransition> {
        self.resolve.values()
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

pub struct AttachmentsBuilder<C: AttachmentList, D: AttachmentList, R: AttachmentList> {
    color: C,
    depth_stencil: D,
    resolve: R,
}

impl AttachmentsBuilder<AttachmentTerminator, AttachmentTerminator, AttachmentTerminator> {
    pub fn new() -> Self {
        Self {
            color: AttachmentTerminator {},
            depth_stencil: AttachmentTerminator {},
            resolve: AttachmentTerminator {},
        }
    }
}

impl<C: AttachmentList, D: AttachmentList, R: AttachmentList> AttachmentsBuilder<C, D, R> {
    pub fn push_color<N: Attachment>(
        self,
        view: vk::ImageView,
    ) -> AttachmentsBuilder<AttachmentNode<N, C>, D, R> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentsBuilder {
            color: AttachmentNode {
                view,
                next: color,
                _phantom: PhantomData,
            },
            depth_stencil,
            resolve,
        }
    }

    pub fn push_depth_stencil<N: Attachment>(
        self,
        view: vk::ImageView,
    ) -> AttachmentsBuilder<C, AttachmentNode<N, D>, R> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentsBuilder {
            color,
            depth_stencil: AttachmentNode {
                view,
                next: depth_stencil,
                _phantom: PhantomData,
            },
            resolve,
        }
    }

    pub fn push_resolve<N: Attachment>(
        self,
        view: vk::ImageView,
    ) -> AttachmentsBuilder<C, D, AttachmentNode<N, R>> {
        let Self {
            color,
            depth_stencil,
            resolve,
        } = self;
        AttachmentsBuilder {
            color,
            depth_stencil,
            resolve: AttachmentNode {
                view,
                next: resolve,
                _phantom: PhantomData,
            },
        }
    }

    pub fn get_attachments(&self) -> Vec<vk::ImageView> {
        self.resolve
            .values()
            .into_iter()
            .chain(self.depth_stencil.values())
            .chain(self.color.values())
            .rev()
            .collect()
    }
}

pub trait Attachments: 'static {
    type Color: AttachmentList;
    type DepthStencil: AttachmentList;
    type Resolve: AttachmentList;
}

pub type Builder<A> = AttachmentsBuilder<
    <A as Attachments>::Color,
    <A as Attachments>::DepthStencil,
    <A as Attachments>::Resolve,
>;

pub type References<A> = AttachmentReferenceBuilder<
    <<A as Attachments>::Color as AttachmentList>::ReferenceListType,
    <<A as Attachments>::DepthStencil as AttachmentList>::ReferenceListType,
    <<A as Attachments>::Resolve as AttachmentList>::ReferenceListType,
>;

pub type Transitions<A> = AttachmentTransitionBuilder<
    <<A as Attachments>::Color as AttachmentList>::TransitionListType,
    <<A as Attachments>::DepthStencil as AttachmentList>::TransitionListType,
    <<A as Attachments>::Resolve as AttachmentList>::TransitionListType,
>;

pub type Clear<A> = ClearValueBuilder<
    <<A as Attachments>::Color as AttachmentList>::ClearListType,
    <<A as Attachments>::DepthStencil as AttachmentList>::ClearListType,
>;

#[derive(Debug)]
pub struct Framebuffer<A: Attachments> {
    pub framebuffer: vk::Framebuffer,
    _phantom: PhantomData<A>,
}

impl<A: Attachments> Clone for Framebuffer<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Attachments> Copy for Framebuffer<A> {}

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
            _phantom: PhantomData,
        })
    }

    pub fn destroy_framebuffer<A: Attachments>(&self, framebuffer: &mut Framebuffer<A>) {
        unsafe {
            self.device
                .destroy_framebuffer(framebuffer.framebuffer, None);
        }
    }
}
