use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u64,
    pub title: String,
    pub body: Option<String>,
    pub kind: NotificationKind,
    remaining: Option<Duration>,
}

impl Notification {
    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

}

#[derive(Debug, Clone)]
pub struct NotificationRequest {
    pub title: String,
    pub body: Option<String>,
    pub kind: NotificationKind,
    pub timeout: Option<Duration>,
}

impl NotificationRequest {
    pub fn title(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            kind: NotificationKind::Info,
            timeout: Some(Duration::from_secs(4)),
        }
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn kind(mut self, kind: NotificationKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Debug, Default)]
pub struct NotificationCenter {
    next_id: u64,
    notifications: Vec<Notification>,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify(&mut self, request: NotificationRequest) -> u64 {
        let id = self.allocate_id();
        let notification = Notification {
            id,
            title: request.title,
            body: request.body,
            kind: request.kind,
            remaining: request.timeout,
        };
        self.notifications.push(notification);
        id
    }

    pub fn dismiss(&mut self, id: u64) {
        self.notifications.retain(|notification| notification.id != id);
    }

    pub fn tick(&mut self, delta: Duration) {
        if delta.is_zero() {
            return;
        }

        for notification in &mut self.notifications {
            if let Some(mut remaining) = notification.remaining {
                remaining = remaining.saturating_sub(delta);
                notification.remaining = Some(remaining);
            }
        }

        self.notifications
            .retain(|notification| match notification.remaining {
                Some(remaining) if remaining.is_zero() => false,
                _ => true,
            });
    }

    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    pub fn has_active(&self) -> bool {
        !self.notifications.is_empty()
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }
}
