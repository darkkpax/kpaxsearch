use std::cell::RefCell;
use windows::{
    core::Interface,
    System::{DispatcherQueue, DispatcherQueueController},
    Win32::{
        Foundation::HWND,
        System::WinRT::{
            Composition::ICompositorDesktopInterop, CreateDispatcherQueueController,
            DispatcherQueueOptions, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT,
        },
    },
    UI::{
        Color,
        Composition::{
            CompositionBrush, CompositionColorBrush, CompositionRoundedRectangleGeometry,
            Compositor, ContainerVisual, Desktop::DesktopWindowTarget, SpriteVisual,
        },
    },
};
use windows_numerics::Vector2;

thread_local! { static QUEUE:RefCell<Option<DispatcherQueueController>>=const {RefCell::new(None)}; }

pub struct Backdrop {
    _target: DesktopWindowTarget,
    _root: ContainerVisual,
    background: SpriteVisual,
    tint: SpriteVisual,
    geometry: CompositionRoundedRectangleGeometry,
    host: CompositionBrush,
    color: CompositionColorBrush,
    // Release the compositor session after its target and visuals.
    _compositor: Compositor,
}

impl Backdrop {
    pub fn new(handle: isize) -> windows::core::Result<Self> {
        if DispatcherQueue::GetForCurrentThread().is_err() {
            let controller = unsafe {
                CreateDispatcherQueueController(DispatcherQueueOptions {
                    dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                    threadType: DQTYPE_THREAD_CURRENT,
                    apartmentType: DQTAT_COM_STA,
                })
            }?;
            QUEUE.with(|slot| *slot.borrow_mut() = Some(controller));
        }
        let compositor = Compositor::new()?;
        let interop: ICompositorDesktopInterop = compositor.cast()?;
        let target = unsafe { interop.CreateDesktopWindowTarget(HWND(handle as *mut _), false) }?;
        let root = compositor.CreateContainerVisual()?;
        let geometry = compositor.CreateRoundedRectangleGeometry()?;
        let clip = compositor.CreateGeometricClipWithGeometry(&geometry)?;
        root.SetClip(&clip)?;
        let background = compositor.CreateSpriteVisual()?;
        let host: CompositionBrush = compositor.CreateHostBackdropBrush()?.cast()?;
        background.SetBrush(&host)?;
        let tint = compositor.CreateSpriteVisual()?;
        let color = compositor.CreateColorBrush()?;
        tint.SetBrush(&color)?;
        root.Children()?.InsertAtBottom(&background)?;
        root.Children()?.InsertAtTop(&tint)?;
        target.SetRoot(&root)?;
        Ok(Self {
            _compositor: compositor,
            _target: target,
            _root: root,
            background,
            tint,
            geometry,
            host,
            color,
        })
    }

    pub fn resize(&self, width: i32, height: i32, diameter: i32) -> windows::core::Result<()> {
        let size = Vector2 {
            X: width as f32,
            Y: height as f32,
        };
        self._root.SetSize(size)?;
        self.background.SetSize(size)?;
        self.tint.SetSize(size)?;
        self.geometry.SetSize(size)?;
        self.geometry.SetCornerRadius(Vector2 {
            X: diameter as f32 / 2.,
            Y: diameter as f32 / 2.,
        })
    }

    pub fn appearance(
        &self,
        material: &str,
        dark: bool,
        density: u32,
    ) -> windows::core::Result<()> {
        let alpha = if material == "none" {
            255
        } else {
            match material {
                "blur" => 25 + density.min(100) * 95 / 100,
                "frosted" => 130 + density.min(100) * 100 / 100,
                _ => 65 + density.min(100) * 110 / 100,
            }
        } as u8;
        let color = if dark {
            Color {
                A: alpha,
                R: 22,
                G: 23,
                B: 26,
            }
        } else {
            Color {
                A: alpha,
                R: 247,
                G: 248,
                B: 250,
            }
        };
        self.color.SetColor(color)?;
        self.background.SetBrush(&self.host)?;
        self.background
            .SetOpacity(if material == "none" { 0. } else { 1. })
    }
}
