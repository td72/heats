use heats_core::platform::macos::focus_window;

fn main() {
    let pid_str = std::env::args()
        .nth(1)
        .expect("usage: heats-focus-window <pid>");

    let pid: i32 = pid_str.parse().expect("invalid pid: expected a number");

    focus_window(pid);
}
