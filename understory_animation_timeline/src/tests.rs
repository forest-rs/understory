// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{AnimationTimeline, ManualTimeline, TimelineTime};

#[test]
fn manual_timeline_is_seekable() {
    let mut timeline = ManualTimeline::new();

    assert_eq!(timeline.current_time(&()), None);

    timeline.seek(TimelineTime::from_duration(40));
    assert_eq!(
        timeline.current_time(&()).map(TimelineTime::duration),
        Some(40)
    );

    timeline.seek(TimelineTime::from_duration(90));
    assert_eq!(
        timeline.current_time(&()).map(TimelineTime::duration),
        Some(90)
    );

    timeline.clear();
    assert_eq!(timeline.current_time(&()), None);
}
