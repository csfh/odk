// snippet start
use odk::prelude::*;
use odk::theme::foundation::MotionDuration;

// An application that offers its users a choice adds to the user's own request in
// `shell.toml`; it cannot turn that request off.
fn reduce_motion(cx: &mut App) {
    odk::ui::set_reduced_motion(cx, true);
}

// Durations come from the foundation and are zero when motion is reduced.
fn fade_time() -> std::time::Duration {
    odk::ui::foundation().motion.transition(MotionDuration::Normal)
}
// snippet end
