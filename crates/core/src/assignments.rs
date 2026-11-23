//! Assign named lessons across a finite calendar. The caller supplies the time
//! left after review, and zero minutes on rest and exam days.
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug)]
pub struct LessonWork {
    pub group: String,
    pub minutes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Portion {
    pub lesson: usize,
    pub minutes: u32,
}

pub struct Schedule {
    pub days: Vec<Vec<Portion>>,
    pub unassigned_minutes: u32,
}

pub fn schedule(lessons: &[LessonWork], capacities: &[u32]) -> Schedule {
    let mut groups = BTreeMap::<&str, VecDeque<usize>>::new();
    for (i, lesson) in lessons.iter().enumerate().filter(|(_, l)| l.minutes > 0) {
        groups.entry(&lesson.group).or_default().push_back(i);
    }
    let mut groups = VecDeque::from_iter(groups.into_values());
    let mut order = VecDeque::new();
    while let Some(mut group) = groups.pop_front() {
        if let Some(i) = group.pop_front() {
            order.push_back((i, lessons[i].minutes));
        }
        if !group.is_empty() {
            groups.push_back(group);
        }
    }
    let mut remaining: u32 = lessons.iter().map(|l| l.minutes).sum();
    let mut capacity_left: u64 = capacities.iter().map(|&c| u64::from(c)).sum();
    let mut days = Vec::with_capacity(capacities.len());
    for &capacity in capacities {
        let mut day = Vec::new();
        if capacity > 0 && remaining > 0 {
            let fair = (u64::from(remaining) * u64::from(capacity)).div_ceil(capacity_left);
            let target = fair.min(u64::from(capacity)) as u32;
            let mut used = 0;
            while used < target {
                let Some((index, left)) = order.front_mut() else { break };
                // Keep a whole lesson together if tomorrow has room to absorb it.
                let room = capacity - used;
                if used > 0 && *left > room && u64::from(remaining) <= capacity_left - u64::from(capacity) {
                    break;
                }
                let take = (*left).min(room);
                day.push(Portion { lesson: *index, minutes: take });
                remaining -= take;
                used += take;
                *left -= take;
                if *left == 0 {
                    order.pop_front();
                }
            }
        }
        capacity_left -= u64::from(capacity);
        days.push(day);
    }
    Schedule { days, unassigned_minutes: remaining }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn work(group: &str, minutes: u32) -> LessonWork {
        LessonWork { group: group.into(), minutes }
    }
    #[test]
    fn rotates_subjects_and_preserves_exam_days_and_capacity() {
        let lessons = [work("a", 4), work("a", 4), work("b", 4), work("c", 4)];
        let s = schedule(&lessons, &[8, 0, 8, 0]);
        assert_eq!(s.days.iter().flatten().map(|p| p.lesson).collect::<Vec<_>>(), [0, 2, 3, 1]);
        assert!(s.days[1].is_empty() && s.days[3].is_empty());
        assert_eq!(s.unassigned_minutes, 0);
    }
    #[test]
    fn missed_work_redistributes_within_the_remaining_time() {
        let lessons = [work("a", 20), work("b", 5)];
        let s = schedule(&lessons, &[10, 0, 8]);
        assert_eq!(s.days[0][0].minutes, 10);
        assert_eq!(s.days[2][0].minutes, 8);
        assert_eq!(s.unassigned_minutes, 7);
        assert_eq!(schedule(&[], &[0, 10]).unassigned_minutes, 0);
    }
}
