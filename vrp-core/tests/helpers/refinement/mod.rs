pub mod population;

use crate::construction::heuristics::InsertionContext;
use crate::helpers::construction::constraints::create_constraint_pipeline_with_timing;
use crate::helpers::models::problem::*;
use crate::helpers::models::solution::{create_route_with_activities, test_tour_activity_with_job};
use crate::models::problem::{create_matrix_transport_cost, Job, Jobs, MatrixData};
use crate::models::solution::{Registry, Route};
use crate::models::{Problem, Solution};
use crate::refinement::mutation::{Recreate, RecreateWithCheapest};
use crate::refinement::objectives::MultiObjective;
use crate::refinement::RefinementContext;
use crate::utils::Random;
use std::sync::Arc;

/// Creates initial solution using cheapest insertion
pub fn create_with_cheapest(problem: Arc<Problem>, random: Arc<dyn Random + Send + Sync>) -> InsertionContext {
    RecreateWithCheapest::default()
        .run(&mut RefinementContext::new(problem.clone()), InsertionContext::new(problem, random))
}

/// Generates problem and solution which has routes distributed uniformly, e.g.:
/// r0 r1 r2 r3
/// -----------
/// 0  4   8 12
/// 1  5   9 13
/// 2  6  10 14
/// 3  7  11 15
pub fn generate_matrix_routes(rows: usize, cols: usize) -> (Problem, Solution) {
    let fleet = Arc::new(
        FleetBuilder::default()
            .add_driver(test_driver_with_costs(empty_costs()))
            .add_vehicles((0..cols).map(|i| test_vehicle_with_id(i.to_string().as_str())).collect())
            .build(),
    );
    let registry = Registry::new(&fleet);

    let mut routes: Vec<Route> = Default::default();
    let mut jobs: Vec<Job> = Default::default();

    (0..cols).for_each(|i| {
        routes.push(create_route_with_activities(&fleet, i.to_string().as_str(), Default::default()));
        (0..rows).for_each(|j| {
            let index = i * rows + j;

            let single =
                test_single_with_id_and_location(["c".to_string(), index.to_string()].concat().as_str(), Some(index));
            let route = routes.get_mut(i).unwrap();
            jobs.push(Job::Single(single.clone()));

            let mut activity = test_tour_activity_with_job(single);
            activity.place.location = index;

            route.tour.insert_last(activity);
        });
    });

    let matrix_values = generate_matrix(rows, cols);
    let matrix_data = MatrixData::new(0, matrix_values.clone(), matrix_values);
    let transport = create_matrix_transport_cost(vec![matrix_data]).unwrap();
    let jobs = Jobs::new(&fleet, jobs, &transport);

    let problem = Problem {
        fleet,
        jobs: Arc::new(jobs),
        locks: vec![],
        constraint: Arc::new(create_constraint_pipeline_with_timing()),
        activity: Arc::new(TestActivityCost::default()),
        transport,
        objective: Arc::new(MultiObjective::default()),
        extras: Arc::new(Default::default()),
    };

    let solution = Solution { registry, routes, unassigned: Default::default(), extras: Arc::new(Default::default()) };

    (problem, solution)
}

fn generate_matrix(rows: usize, cols: usize) -> Vec<f64> {
    let size = cols * rows;
    let mut data = vec![0.; size * size];

    (0..size).for_each(|i| {
        let (left1, right1) = (i / rows, i % rows);
        ((i + 1)..size).for_each(|j| {
            let (left2, right2) = (j / rows, j % rows);
            let left_delta = left1 as f64 - left2 as f64;
            let right_delta = right1 as f64 - right2 as f64;

            let value = (left_delta * left_delta + right_delta * right_delta).sqrt();

            let sym_j = (j as i32 + (j as i32 - i as i32) * (size as i32 - 1)) as usize;

            data[i * size + j] = value;
            data[i * size + sym_j] = value;
        });
    });

    data
}
