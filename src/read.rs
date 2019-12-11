use array2d::{Array2D};
use std::io;
use indicatif::{ProgressBar};
use crate::{Result, Problem, CityProblem, AirProblem};

pub fn read_problem(input: &mut dyn io::BufRead,
  bar: &ProgressBar) -> Result<Problem>
{
  bar.set_message("reading cities and depos");
  let mut problem = read_problem_cities_depos(input)?;
  bar.set_message("reading trucks");
  read_problem_trucks(input, &mut problem)?;
  bar.set_message("reading airplanes");
  read_problem_airplanes(input, &mut problem)?;
  bar.set_message("reading parcels");
  read_problem_parcels(input, &mut problem)?;
  Ok(problem)
}

fn read_problem_cities_depos(input: &mut dyn io::BufRead) -> Result<Problem> {
  let city_count = read_int(input)?;
  let depo_count = read_int(input)?;

  // read depos
  let mut city_depo_ids: Vec<Vec<u32>> = vec![Vec::new(); city_count as usize];
  let mut depos: Vec<(u32, u32)> = Vec::with_capacity(depo_count as usize);
  for depo_id in 0..depo_count {
    let depo_city = read_int(input)?;
    if depo_city >= city_count {
      return Err("Read invalid city")?;
    }

    let depo_idx = city_depo_ids[depo_city as usize].len() as u32;
    city_depo_ids[depo_city as usize].push(depo_id);
    depos.push((depo_city, depo_idx));
  }

  // read airports
  let mut city_airport_depos: Vec<Option<u32>> = vec![None; city_count as usize];
  for _ in 0..city_count {
    let airport_depo_id = read_int(input)?;
    if airport_depo_id >= depo_count {
      return Err("Read invalid depo as airport")?;
    }

    let (city, depo_idx) = depos[airport_depo_id as usize];
    if city_airport_depos[city as usize].is_some() {
      return Err("A city has multiple airports")?;
    }
    city_airport_depos[city as usize] = Some(depo_idx);
  }

  // build the Problem
  let city_problems = (0..city_count).map(|city| {
    let depo_count = city_depo_ids[city as usize].len();
    CityProblem {
      depo_ids: city_depo_ids[city as usize].clone(),
      airport_depo: city_airport_depos[city as usize].unwrap(),
      truck_ids: Vec::new(),
      truck_depos: Vec::new(),
      inner_parcel_ids: Array2D::filled_with(Vec::new(), depo_count, depo_count),
      outbound_parcel_ids: vec![Vec::new(); depo_count],
      inbound_parcel_ids: vec![Vec::new(); depo_count],
      parcel_count: 0,
    }
  }).collect();

  let air_problem = AirProblem {
    airport_ids: city_airport_depos.iter().enumerate()
      .map(|(city, depo_idx)| city_depo_ids[city][depo_idx.unwrap() as usize]).collect(),
    airplane_airports: Vec::new(),
    parcel_ids: Array2D::filled_with(Vec::new(),city_count as usize,city_count as usize),
  };

  Ok(Problem { depos, cities: city_problems, air: air_problem, parcel_count: 0 })
}

fn read_problem_trucks(input: &mut dyn io::BufRead, problem: &mut Problem) -> Result<()> {
  let truck_count = read_int(input)?;
  let depo_count = problem.depos.len() as u32;
  let city_count = problem.cities.len() as u32;

  for truck_id in 0..truck_count {
    let depo_id = read_int(input)?;
    if depo_id >= depo_count {
      return Err("Read invalid depo as truck position")?;
    }

    let (city, depo_idx) = problem.depos[depo_id as usize];
    problem.cities[city as usize].truck_ids.push(truck_id);
    problem.cities[city as usize].truck_depos.push(depo_idx);
  }

  for city in 0..city_count {
    if problem.cities[city as usize].truck_ids.is_empty() {
      return Err("A city has no trucks")?;
    }
  }

  Ok(())
}

fn read_problem_airplanes(input: &mut dyn io::BufRead,
  problem: &mut Problem) -> Result<()>
{
  let airplane_count = read_int(input)?;
  let depo_count = problem.depos.len() as u32;

  for _airplane_id in 0..airplane_count {
    let airport_depo_id = read_int(input)?;
    if airport_depo_id >= depo_count {
      return Err("Read invalid depo as airplane position")?;
    }

    let (city, depo_idx) = problem.depos[airport_depo_id as usize];
    if problem.cities[city as usize].airport_depo != depo_idx {
      return Err("Read depo that is not an airport as airplane position")?;
    }

    problem.air.airplane_airports.push(city);
  }

  if airplane_count == 0 {
    return Err("There are no airplanes")?;
  }

  Ok(())
}

fn read_problem_parcels(input: &mut dyn io::BufRead, problem: &mut Problem) -> Result<()> {
  let parcel_count = read_int(input)?;
  let depo_count = problem.depos.len() as u32;

  for parcel_id in 0..parcel_count {
    let (src_id, tgt_id) = read_int_pair(input)?;
    if src_id >= depo_count || tgt_id >= depo_count {
      return Err("Read invalid depo as parcel source/target")?;
    }

    let (src_city, src_depo) = problem.depos[src_id as usize];
    let (tgt_city, tgt_depo) = problem.depos[tgt_id as usize];

    if src_city != tgt_city {
      {
        let p = &mut problem.cities[src_city as usize];
        p.outbound_parcel_ids[src_depo as usize].push(parcel_id);
        p.parcel_count += 1;
      }
      {
        let p = &mut problem.cities[tgt_city as usize];
        p.inbound_parcel_ids[tgt_depo as usize].push(parcel_id);
        p.parcel_count += 1;
      }
      problem.air.parcel_ids[(src_city as usize, tgt_city as usize)].push(parcel_id);
    } else {
      let p = &mut problem.cities[src_city as usize];
      p.inner_parcel_ids[(src_depo as usize, tgt_depo as usize)].push(parcel_id);
      p.parcel_count += 1;
    }

    problem.parcel_count += 1;
  }

  Ok(())
}



fn read_int(input: &mut dyn io::BufRead) -> Result<u32> {
  Ok(read_line(input)?.trim().parse()?)
}

fn read_int_pair(input: &mut dyn io::BufRead) -> Result<(u32, u32)> {
  let line = read_line(input)?;
  let mut fields = line.split_whitespace();
  let value0 = fields.next()
    .ok_or("Expected at least two integers, got none")?.parse()?;
  let value1 = fields.next()
    .ok_or("Expected at least two integers, got one")?.parse()?;
  Ok((value0, value1))
}

fn read_line(input: &mut dyn io::BufRead) -> Result<String> {
  let mut line = String::new();
  loop {
    if input.read_line(&mut line)? == 0 {
      return Err("Expected integer, got end of file")?;
    }
    let is_comment = line.starts_with("%");
    let is_blank = line.chars().all(|c| c.is_whitespace());
    if !is_comment && !is_blank {
      return Ok(line)
    } else {
      line.clear();
    }
  }
}
