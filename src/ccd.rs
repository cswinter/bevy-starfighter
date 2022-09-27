// From https://github.com/jcornaz/heron/issues/199#issuecomment-1090279292
use bevy::prelude::*;
use heron::rapier_plugin::{
    convert::IntoRapier, rapier2d::prelude::RigidBodySet, RigidBodyHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum Ccd {
    Disabled,
    Enabled,
}

impl Default for Ccd {
    fn default() -> Self {
        Self::Disabled
    }
}

pub struct CcdPhysicsPlugin;

impl Plugin for CcdPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_to_stage(CoreStage::First, update_ccd_for_body);
    }
}

#[allow(clippy::type_complexity)]
fn update_ccd_for_body(
    mut rigid_bodies: ResMut<RigidBodySet>,
    new_handles: Query<
        (&RigidBodyHandle, &Ccd),
        Or<(Added<RigidBodyHandle>, Changed<Ccd>)>,
    >,
) {
    for (&handle, &ccd) in new_handles.iter() {
        if let Some(body) = rigid_bodies.get_mut(handle.into_rapier()) {
            let enable = match ccd {
                Ccd::Enabled => true,
                Ccd::Disabled => false,
            };
            body.enable_ccd(enable);
        }
    }
}
