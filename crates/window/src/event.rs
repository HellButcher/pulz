use crate::{Point2, Size2, WindowId};

#[derive(Debug, Clone)]
pub struct WindowResized {
    pub id: WindowId,
    pub size: Size2,
}

#[derive(Debug, Clone)]
pub enum WindowEvent {
    Created(WindowId),
    CloseRequested(WindowId),
    Closed(WindowId),
}

#[derive(Debug, Clone)]
pub enum CursorEvent {
    Enter(WindowId),
    Move(WindowId, Point2),
    Leave(WindowId),
}
