use std::time::SystemTime;


pub type Millis = u128;

pub struct Notification {
    pub text: String,
    show_time_millis: Millis,
    start_time: SystemTime,
}

impl Notification {
    pub fn new(text: &str, show_time_millis: Millis) -> Notification {
        let text = text.to_string();
        Notification {
            text,
            show_time_millis,
            start_time: SystemTime::now(),
        }
    }

    pub fn has_finished(&self) -> bool {
        millis_since(self.start_time) > self.show_time_millis
    }
}

pub fn millis_since(time: SystemTime) -> Millis {
    let elapsed = SystemTime::now().duration_since(time);
    if elapsed.is_err() { return 0; } // _now_ is earlier than _time_ => assume 0
    elapsed.unwrap().as_millis()
}
