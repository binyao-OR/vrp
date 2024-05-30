//! A features to put some extra limits on tour.

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/tour_limits_test.rs"]
mod tour_limits_test;

use hashbrown::HashSet;
use super::*;
use crate::construction::enablers::{calculate_travel_delta, ScheduleKeys};
use crate::models::common::{Distance, Duration};
use crate::models::problem::{Actor, TransportCost};

/// A function which returns activity size limit for given actor.
pub type ActivitySizeResolver = Arc<dyn Fn(&Actor) -> Option<usize> + Sync + Send>;
/// A function to resolve travel limit.
pub type TravelLimitFn<T> = Arc<dyn Fn(&Actor) -> Option<T> + Send + Sync>;

/// Combines all keys needed for tour limits feature usage.
#[derive(Clone)]
pub struct TourLimitKeys {
    /// A key to track duration limit.
    pub duration_key: StateKey,
    /// Schedule keys.
    pub schedule_keys: ScheduleKeys,
    /// A distance constraint violation code.
    pub distance_code: ViolationCode,
    /// A duration constraint violation code.
    pub duration_code: ViolationCode,
}

/// Creates a limit for activity amount in a tour.
/// This is a hard constraint.
pub fn create_activity_limit_feature(
    name: &str,
    code: ViolationCode,
    limit_func: ActivitySizeResolver,
) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(ActivityLimitConstraint { code, limit_fn: limit_func })
        .build()
}

/// Creates a limit for location amount in a tour.
/// This is a hard constraint.
pub fn create_location_limit_feature(
    name: &str,
    code: ViolationCode,
    limit_func: ActivitySizeResolver,
) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(LocationLimitConstraint { code, limit_fn: limit_func })
        .build()
}

/// Creates a travel limits such as distance and/or duration.
/// This is a hard constraint.
pub fn create_travel_limit_feature(
    name: &str,
    transport: Arc<dyn TransportCost + Send + Sync>,
    tour_distance_limit_fn: TravelLimitFn<Distance>,
    tour_duration_limit_fn: TravelLimitFn<Duration>,
    limit_keys: TourLimitKeys,
) -> Result<Feature, GenericError> {
    let duration_key = limit_keys.duration_key;

    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(TravelLimitConstraint {
            transport,
            tour_distance_limit_fn,
            tour_duration_limit_fn: tour_duration_limit_fn.clone(),
            limit_keys,
        })
        .with_state(TravelLimitState { tour_duration_limit_fn, duration_key, state_keys: vec![] })
        .build()
}

struct ActivityLimitConstraint {
    code: ViolationCode,
    limit_fn: ActivitySizeResolver,
}

impl FeatureConstraint for ActivityLimitConstraint {
    fn evaluate(&self, move_ctx: &MoveContext<'_>) -> Option<ConstraintViolation> {
        match move_ctx {
            MoveContext::Route { route_ctx, job, .. } => {
                (self.limit_fn)(route_ctx.route().actor.as_ref()).and_then(|limit| {
                    let tour_activities = route_ctx.route().tour.job_activity_count();

                    let job_activities = match job {
                        Job::Single(_) => 1,
                        Job::Multi(multi) => multi.jobs.len(),
                    };

                    if tour_activities + job_activities > limit {
                        ConstraintViolation::fail(self.code)
                    } else {
                        ConstraintViolation::success()
                    }
                })
            }
            MoveContext::Activity { .. } => ConstraintViolation::success(),
        }
    }

    fn merge(&self, source: Job, _: Job) -> Result<Job, ViolationCode> {
        Ok(source)
    }
}

struct LocationLimitConstraint {
    code: ViolationCode,
    limit_fn: ActivitySizeResolver,
}

impl FeatureConstraint for crate::construction::features::tour_limits::LocationLimitConstraint {
    fn evaluate(&self, move_ctx: &MoveContext<'_>) -> Option<ConstraintViolation> {
        match move_ctx {
            MoveContext::Route { route_ctx, job, .. } => {
                (self.limit_fn)(route_ctx.route().actor.as_ref()).and_then(|limit| {

                    let tour_place_set = {
                        let mut unique_places = HashSet::new();
                        for activity in &route_ctx.route().tour.activities {
                            unique_places.insert(activity.place.location);
                        }
                        unique_places
                    };

                    let job_place_set_option = match job {
                        Job::Single(single) => {
                            let mut unique_places = HashSet::new();
                            for place in &single.places {
                                unique_places.insert(place.location);
                            }
                            unique_places
                        },
                        Job::Multi(multi) => {
                            let mut unique_places = HashSet::new();
                            for single in &multi.jobs {
                                for place in &single.places {
                                    unique_places.insert(place.location);
                                }
                            }
                            unique_places
                        },
                    };

                    let job_place_set: HashSet<Location> = job_place_set_option.iter()
                        .chain(job_place_set_option.iter())
                        .filter_map(|x| *x)
                        .collect();

                    let total_unique_places: HashSet<_> = tour_place_set.union(&job_place_set).collect();

                    if total_unique_places.len() > limit {
                        ConstraintViolation::fail(self.code)
                    } else {
                        ConstraintViolation::success()
                    }
                })
            }
            MoveContext::Activity { .. } => ConstraintViolation::success(),
        }
    }

    fn merge(&self, source: Job, _: Job) -> Result<Job, ViolationCode> {
        Ok(source)
    }
}

struct TravelLimitConstraint {
    transport: Arc<dyn TransportCost + Send + Sync>,
    tour_distance_limit_fn: TravelLimitFn<Distance>,
    tour_duration_limit_fn: TravelLimitFn<Duration>,
    limit_keys: TourLimitKeys,
}

impl TravelLimitConstraint {
    fn calculate_travel(&self, route_ctx: &RouteContext, activity_ctx: &ActivityContext) -> (Distance, Duration) {
        calculate_travel_delta(route_ctx, activity_ctx, self.transport.as_ref())
    }
}

impl FeatureConstraint for TravelLimitConstraint {
    fn evaluate(&self, move_ctx: &MoveContext<'_>) -> Option<ConstraintViolation> {
        match move_ctx {
            MoveContext::Route { .. } => None,
            MoveContext::Activity { route_ctx, activity_ctx } => {
                let schedule_keys = &self.limit_keys.schedule_keys;
                let tour_distance_limit = (self.tour_distance_limit_fn)(route_ctx.route().actor.as_ref());
                let tour_duration_limit = (self.tour_duration_limit_fn)(route_ctx.route().actor.as_ref());

                if tour_distance_limit.is_some() || tour_duration_limit.is_some() {
                    let (change_distance, change_duration) = self.calculate_travel(route_ctx, activity_ctx);

                    if let Some(distance_limit) = tour_distance_limit {
                        let curr_dis =
                            route_ctx.state().get_route_state(schedule_keys.total_distance).cloned().unwrap_or(0.);
                        let total_distance = curr_dis + change_distance;
                        if distance_limit < total_distance {
                            return ConstraintViolation::skip(self.limit_keys.distance_code);
                        }
                    }

                    if let Some(duration_limit) = tour_duration_limit {
                        let curr_dur =
                            route_ctx.state().get_route_state(schedule_keys.total_duration).cloned().unwrap_or(0.);
                        let total_duration = curr_dur + change_duration;
                        if duration_limit < total_duration {
                            return ConstraintViolation::skip(self.limit_keys.duration_code);
                        }
                    }
                }

                None
            }
        }
    }

    fn merge(&self, source: Job, _: Job) -> Result<Job, ViolationCode> {
        Ok(source)
    }
}

struct TravelLimitState {
    tour_duration_limit_fn: TravelLimitFn<Duration>,
    duration_key: StateKey,
    state_keys: Vec<StateKey>,
}

impl FeatureState for TravelLimitState {
    fn accept_insertion(&self, _: &mut SolutionContext, _: usize, _: &Job) {}

    fn accept_route_state(&self, route_ctx: &mut RouteContext) {
        if let Some(limit_duration) = (self.tour_duration_limit_fn)(route_ctx.route().actor.as_ref()) {
            route_ctx.state_mut().put_route_state(self.duration_key, limit_duration);
        }
    }

    fn accept_solution_state(&self, _: &mut SolutionContext) {}

    fn state_keys(&self) -> Iter<StateKey> {
        self.state_keys.iter()
    }
}
