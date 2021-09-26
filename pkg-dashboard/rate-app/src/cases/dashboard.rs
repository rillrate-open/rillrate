use super::state::{CasesState, SCENE};
use anyhow::Error;
use rate_ui::shared_object::{DataChanged, SharedObject};
use rate_ui::widget::{Context, NotificationHandler, Widget, WidgetRuntime};
use yew::{html, Html};

pub type Dashboard = WidgetRuntime<DashboardWidget>;

pub struct DashboardWidget {
    scene: SharedObject<CasesState>,
}

impl Default for DashboardWidget {
    fn default() -> Self {
        Self {
            scene: SCENE.with(SharedObject::clone),
        }
    }
}

impl Widget for DashboardWidget {
    type Event = ();
    type Tag = ();
    type Properties = ();
    type Meta = ();

    fn init(&mut self, ctx: &mut Context<Self>) {
        self.scene.subscribe(ctx);
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let state = self.scene.read();
        if let Some(layout) = state.get_layout_tab() {
            html! {
                <super::LayoutViewer layout_tab=layout.clone() />
            }
        } else {
            html! {
                <p>{ "Loading" }</p>
            }
        }
    }
}

impl NotificationHandler<DataChanged<CasesState>> for DashboardWidget {
    fn handle(
        &mut self,
        _event: DataChanged<CasesState>,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        ctx.redraw();
        Ok(())
    }
}
