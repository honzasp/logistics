use std::io;
use indicatif::{ProgressBar};
use crate::{Result, Plan, parcel_plan::{Action}};

#[derive(Debug, Copy, Clone)]
enum Kind { Truck, Airplane }

pub fn write_plan(output: &mut dyn io::Write,
  plan: &Plan, bar: &ProgressBar) -> Result<()>
{
  bar.set_message("writing truck actions (1)");
  for action in plan.truck_actions_1.iter() {
    write_action(output, action, Kind::Truck)?;
  }

  bar.set_message("writing air actions (2)");
  for action in plan.airplane_actions_2.iter() {
    write_action(output, action, Kind::Airplane)?;
  }

  bar.set_message("writing truck actions (3)");
  for action in plan.truck_actions_3.iter() {
    write_action(output, action, Kind::Truck)?;
  }

  Ok(())
}

fn write_action(output: &mut dyn io::Write, action: &Action, kind: Kind) -> Result<()> {
  let (go, load, unload) = match kind {
    Kind::Truck => ("drive", "load", "unload"),
    Kind::Airplane => ("fly", "pickup", "dropoff"),
  };
  match action {
    Action::Go { vehicle_id, src_id: _, tgt_id } =>
      write!(output, "{} {} {}\n", go, vehicle_id, tgt_id)?,
    Action::Load { vehicle_id, parcel_id } =>
      write!(output, "{} {} {}\n", load, vehicle_id, parcel_id)?,
    Action::Unload { vehicle_id, parcel_id } =>
      write!(output, "{} {} {}\n", unload, vehicle_id, parcel_id)?,
  };
  Ok(())
}
