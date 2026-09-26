use super::labels::{action_label, control_label};
use super::{
    AxisDirection, GamepadAxis, GamepadButton, PhysicalAction, PhysicalBindings, PhysicalControl,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum BindingTarget {
    Control(PhysicalControl),
    // Controllers can report a trigger as a button, an axis, or both. Editing
    // the pictured trigger updates both reports atomically in the UI draft.
    Trigger { right: bool },
}

impl BindingTarget {
    pub(super) fn from_control(control: PhysicalControl) -> Self {
        match control {
            PhysicalControl::Gamepad {
                button: GamepadButton::LeftTrigger,
            }
            | PhysicalControl::Axis {
                axis: GamepadAxis::LeftTrigger,
                ..
            } => Self::Trigger { right: false },
            PhysicalControl::Gamepad {
                button: GamepadButton::RightTrigger,
            }
            | PhysicalControl::Axis {
                axis: GamepadAxis::RightTrigger,
                ..
            } => Self::Trigger { right: true },
            _ => Self::Control(control),
        }
    }

    pub(super) fn controls(self) -> impl Iterator<Item = PhysicalControl> {
        let controls = match self {
            Self::Control(control) => [Some(control), None],
            Self::Trigger { right } => [
                Some(PhysicalControl::Gamepad {
                    button: if right {
                        GamepadButton::RightTrigger
                    } else {
                        GamepadButton::LeftTrigger
                    },
                }),
                Some(PhysicalControl::Axis {
                    axis: if right {
                        GamepadAxis::RightTrigger
                    } else {
                        GamepadAxis::LeftTrigger
                    },
                    direction: AxisDirection::Positive,
                }),
            ],
        };
        controls.into_iter().flatten()
    }

    pub(super) fn label(self) -> String {
        match self {
            Self::Control(control) => control_label(control),
            Self::Trigger { right: false } => "L2 / LT".to_owned(),
            Self::Trigger { right: true } => "R2 / RT".to_owned(),
        }
    }

    pub(super) fn actions(
        self,
        bindings: &PhysicalBindings,
    ) -> impl Iterator<Item = PhysicalAction> + Clone + use<> {
        let mut actions = self
            .controls()
            .filter_map(|control| bindings.resolve(control));
        let first = actions.next();
        let second = actions.next().filter(|action| Some(*action) != first);
        [first, second].into_iter().flatten()
    }

    pub(super) fn assignment(self, bindings: &PhysicalBindings) -> String {
        let mut actions = self.actions(bindings).map(action_label);
        match (actions.next(), actions.next()) {
            (Some(first), Some(second)) => format!("{first} / {second}"),
            (Some(first), None) => first.to_owned(),
            _ => "Unassigned".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum ControlGroup {
    Dpad,
    LeftStick,
    RightStick,
}

impl ControlGroup {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Dpad => "D-pad",
            Self::LeftStick => "Left stick",
            Self::RightStick => "Right stick",
        }
    }

    /// Row-major positions in a directional cross. The center is a stick click.
    pub(super) fn controls(self) -> [(usize, &'static str, Option<PhysicalControl>); 5] {
        use AxisDirection::{Negative, Positive};
        use GamepadAxis as A;
        use GamepadButton as B;
        let button = |button| Some(PhysicalControl::Gamepad { button });
        let axis = |axis, direction| Some(PhysicalControl::Axis { axis, direction });
        let controls = match self {
            Self::Dpad => [
                button(B::DpadUp),
                button(B::DpadLeft),
                None,
                button(B::DpadRight),
                button(B::DpadDown),
            ],
            Self::LeftStick => [
                axis(A::LeftY, Negative),
                axis(A::LeftX, Negative),
                button(B::LeftStick),
                axis(A::LeftX, Positive),
                axis(A::LeftY, Positive),
            ],
            Self::RightStick => [
                axis(A::RightY, Negative),
                axis(A::RightX, Negative),
                button(B::RightStick),
                axis(A::RightX, Positive),
                axis(A::RightY, Positive),
            ],
        };
        [
            (1, "Up", controls[0]),
            (3, "Left", controls[1]),
            (4, "Click", controls[2]),
            (5, "Right", controls[3]),
            (7, "Down", controls[4]),
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Picker {
    Group(ControlGroup),
    Binding(BindingTarget),
}

impl Picker {
    pub(super) fn label(self) -> String {
        match self {
            Self::Group(group) => group.label().to_owned(),
            Self::Binding(target) => target.label(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(super) enum ActionPage {
    #[default]
    Phone,
    Keypad,
    App,
}

impl ActionPage {
    pub(super) fn for_action(action: PhysicalAction) -> Self {
        use super::HostAction as H;
        match action {
            PhysicalAction::Phone(
                H::Num0
                | H::Num1
                | H::Num2
                | H::Num3
                | H::Num4
                | H::Num5
                | H::Num6
                | H::Num7
                | H::Num8
                | H::Num9
                | H::Star
                | H::Pound,
            ) => Self::Keypad,
            PhysicalAction::Phone(_) => Self::Phone,
            _ => Self::App,
        }
    }
}
