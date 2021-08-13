use anyhow::Error;
use rillrate::gauge::GaugeSpec;
use rillrate::range::Range;
use rillrate::*;
use tokio::time::{sleep, Duration};

const PACKAGE_1: &str = "package-1";
const DASHBOARD_1: &str = "dashboard-1";
const DASHBOARD_2: &str = "dashboard-2";
const DASHBOARD_I: &str = "issues";

const GROUP_1: &str = "group-1";
const GROUP_2: &str = "group-2";

const FIRST_LIMIT: usize = 10;
const SECOND_LIMIT: usize = 50;

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    env_logger::try_init()?;
    let _handle = rillrate::start();

    // Special tracers for checking issues:
    // 1. If `Pulse` has no data a range become intinite and UI app is stucked.
    let _pulse_empty = Pulse::new(
        [PACKAGE_1, DASHBOARD_I, GROUP_1, "pulse-empty"].into(),
        None,
    );
    let long_board = Board::new([PACKAGE_1, DASHBOARD_I, GROUP_2, "long-board"].into());
    long_board.set(
        "Very Long Long Long Long Long Long Long Key",
        "Very Long Long Long Long Long Long Long Long Long Long Value",
    );
    long_board.set(
        "Very Long Long Long Long Long Long Long Key1",
        "Very Long Long Long Long Long Long Long Long Long Long Value",
    );
    long_board.set(
        "Very Long Long Long Long Long Long Long Key2",
        "Very Long Long Long Long Long Long Long Long Long Long Value",
    );
    long_board.set(
        "Very Long Long Long Long Long Long Long Key3",
        "Very Long Long Long Long Long Long Long Long Long Long Value",
    );
    long_board.set(
        "Very-Long-Long-Long-Long-Long-Long-Long-Key3",
        "Very-Long-Long-Long-Long-Long-Long-Long-Long-Long-Long-Value",
    );
    long_board.set(
        "Very::Long::Long::Long::Long::Long::Long::Long::Key3",
        "Very::Long::Long::Long::Long::Long::Long::Long::Long::Long::Long::Value",
    );

    let link = Link::new();
    link.sender();
    let click = Click::new(
        [PACKAGE_1, DASHBOARD_1, GROUP_1, "click-1"].into(),
        "Click Me!",
        link.sender(),
    );
    tokio::spawn(async move {
        let mut rx = link.receiver();
        while let Some(envelope) = rx.recv().await {
            log::warn!("ACTION: {:?}", envelope);
            if envelope.activity.is_action() {
                click.clicked();
            }
        }
    });

    let link = Link::new();
    link.sender();
    let switch = Switch::new(
        [PACKAGE_1, DASHBOARD_1, GROUP_1, "switch-1"].into(),
        "Switch Me!",
        link.sender(),
    );
    tokio::spawn(async move {
        let mut rx = link.receiver();
        while let Some(envelope) = rx.recv().await {
            log::warn!("ACTION: {:?}", envelope);
            if let Some(action) = envelope.activity.to_action() {
                switch.turn(action.turn_on);
            }
        }
    });

    let link = Link::new();
    link.sender();
    let slider = Slider::new(
        [PACKAGE_1, DASHBOARD_1, GROUP_1, "slider-1"].into(),
        "Slide Me!",
        100.0,
        5_000.0,
        100.0,
        link.sender(),
    );
    tokio::spawn(async move {
        let mut rx = link.receiver();
        while let Some(envelope) = rx.recv().await {
            log::warn!("ACTION: {:?}", envelope);
            if let Some(action) = envelope.activity.to_action() {
                slider.set(action.new_value);
            }
        }
    });

    let link = Link::new();
    link.sender();
    let selector = Selector::new(
        [PACKAGE_1, DASHBOARD_1, GROUP_1, "selector-1"].into(),
        "Select Me!",
        vec!["One".into(), "Two".into(), "Three".into()],
        link.sender(),
    );
    tokio::spawn(async move {
        let mut rx = link.receiver();
        while let Some(envelope) = rx.recv().await {
            log::warn!("ACTION: {:?}", envelope);
            if let Some(action) = envelope.activity.to_action() {
                selector.select(action.new_selected);
            }
        }
    });

    // === The main part ===
    // TODO: Improve that busy paths declarations...
    let counter_1 = Counter::new([PACKAGE_1, DASHBOARD_1, GROUP_1, "counter-1"].into(), true);
    let counter_2 = Counter::new([PACKAGE_1, DASHBOARD_1, GROUP_1, "counter-2"].into(), true);
    let counter_3 = Counter::new([PACKAGE_1, DASHBOARD_1, GROUP_1, "counter-3"].into(), true);
    let gauge_1_spec = GaugeSpec {
        pull_ms: None,
        range: Range::new(0.0, FIRST_LIMIT as f64),
    };
    let gauge_1 = Gauge::new(
        [PACKAGE_1, DASHBOARD_1, GROUP_1, "gauge-1"].into(),
        Some(gauge_1_spec),
        true,
    );
    let gauge_2_spec = GaugeSpec {
        pull_ms: None,
        range: Range::new(0.0, SECOND_LIMIT as f64),
    };
    let gauge_2 = Gauge::new(
        [PACKAGE_1, DASHBOARD_1, GROUP_1, "gauge-2"].into(),
        Some(gauge_2_spec),
        true,
    );
    let pulse_1 = Pulse::new([PACKAGE_1, DASHBOARD_2, GROUP_1, "pulse-1"].into(), None);
    let board_1 = Board::new([PACKAGE_1, DASHBOARD_2, GROUP_2, "board-1"].into());
    loop {
        board_1.set("Loop", "First");
        for x in 1..=FIRST_LIMIT {
            gauge_1.set(x as f64);
            counter_1.inc(1);
            counter_2.inc(10);
            counter_3.inc(100);
            pulse_1.add(x as f64);
            sleep(Duration::from_secs(1)).await;
        }
        board_1.set("Loop", "Second");
        let pulse_2 = Pulse::new([PACKAGE_1, DASHBOARD_2, GROUP_1, "pulse-2"].into(), None);
        for x in 1..=SECOND_LIMIT {
            gauge_2.set(x as f64);
            counter_1.inc(1);
            counter_2.inc(10);
            counter_3.inc(100);
            pulse_1.add(x as f64);
            pulse_2.add(x as f64);
            sleep(Duration::from_millis(500 - x as u64 * 10)).await;
        }
        sleep(Duration::from_secs(1)).await;
    }
}
