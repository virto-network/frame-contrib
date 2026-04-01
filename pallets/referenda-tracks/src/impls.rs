use super::*;

use alloc::borrow::Cow;
use frame_support::{ensure, traits::Incrementable};
use sp_core::Get;
use sp_runtime::{traits::Zero, DispatchError, DispatchResult};

type OriginOf<T> = <<T as frame_system::Config>::RuntimeOrigin as OriginTrait>::PalletsOrigin;

impl<T: Config<I>, I: 'static> pallet_referenda::TracksInfo<BalanceOf<T, I>, BlockNumberFor<T, I>>
    for Pallet<T, I>
{
    type Id = T::TrackId;
    type RuntimeOrigin = OriginOf<T>;

    fn tracks(
    ) -> impl Iterator<Item = Cow<'static, Track<Self::Id, BalanceOf<T, I>, BlockNumberFor<T, I>>>>
    {
        Tracks::<T, I>::iter().map(|(group, track, info)| {
            Cow::Owned(Track {
                id: T::TrackId::combine(group, track),
                info,
            })
        })
    }

    fn track_for(origin: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
        OriginToTrackId::<T, I>::get(origin).ok_or(())
    }

    fn track_ids() -> impl Iterator<Item = Self::Id> {
        TracksIds::<T, I>::get().into_iter()
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    #[inline]
    pub fn get_track_info(id: T::TrackId) -> Option<TrackInfoOf<T, I>> {
        let (group, track) = id.split();
        Tracks::<T, I>::get(group, track)
    }

    #[inline]
    pub(crate) fn next_group_track_id() -> Option<T::TrackId> {
        NextGroupId::<T, I>::try_mutate(|id| -> Result<T::TrackId, ()> {
            let new_id = id.increment().ok_or(())?;
            *id = new_id.clone();
            let track = SubTrackIdOf::<T, I>::default();
            Ok(T::TrackId::combine(new_id, track))
        })
        .ok()
    }

    /// Validates the essential fields of a track configuration.
    fn validate_track_info(info: &TrackInfoOf<T, I>) -> DispatchResult {
        ensure!(info.max_deciding > 0, Error::<T, I>::InvalidTrackInfo);
        ensure!(
            !info.decision_period.is_zero(),
            Error::<T, I>::InvalidTrackInfo
        );
        ensure!(
            !info.confirm_period.is_zero(),
            Error::<T, I>::InvalidTrackInfo
        );
        Ok(())
    }

    /// Inserts a new track into the tracks storage.
    pub fn do_insert(
        id: T::TrackId,
        info: TrackInfoOf<T, I>,
        origin: OriginOf<T>,
    ) -> DispatchResult {
        Self::validate_track_info(&info)?;
        let (group, track) = id.split();
        ensure!(
            Self::get_track_info(id).is_none(),
            Error::<T, I>::TrackIdAlreadyExisting
        );
        ensure!(
            OriginToTrackId::<T, I>::get(&origin).is_none(),
            Error::<T, I>::TrackIdAlreadyExisting
        );

        TracksIds::<T, I>::try_mutate(|ids| ids.try_insert(id))
            .map_err(|_| Error::<T, I>::MaxTracksExceeded)?;
        Tracks::<T, I>::set(group, track, Some(info));
        OriginToTrackId::<T, I>::set(origin.clone(), Some(id));
        TrackIdToOrigin::<T, I>::set(id, Some(origin));

        Self::deposit_event(Event::Created { id });
        Ok(())
    }

    #[inline]
    pub(crate) fn next_sub_track_id(group: GroupIdOf<T, I>) -> Option<T::TrackId> {
        NextSubTrackId::<T, I>::try_mutate(group.clone(), |id| -> Result<T::TrackId, ()> {
            let new_id = id.increment().ok_or(())?;
            *id = new_id.clone();
            Ok(T::TrackId::combine(group, new_id))
        })
        .ok()
    }

    pub(crate) fn do_remove(id: T::TrackId) -> frame_support::dispatch::DispatchResult {
        let (group, track) = id.split();
        ensure!(
            Tracks::<T, I>::contains_key(&group, &track),
            Error::<T, I>::TrackIdNotFound
        );
        ensure!(
            pallet_referenda::DecidingCount::<T, I>::get(id) == 0
                && pallet_referenda::TrackQueue::<T, I>::get(id).is_empty(),
            Error::<T, I>::CannotRemove
        );

        let origin = TrackIdToOrigin::<T, I>::get(id);

        Tracks::<T, I>::remove(group, track);
        TrackIdToOrigin::<T, I>::remove(id);
        if let Some(ref origin) = origin {
            OriginToTrackId::<T, I>::remove(origin);
        }
        TracksIds::<T, I>::try_mutate(|tracks_ids| {
            tracks_ids.remove(&id);
            Ok::<(), DispatchError>(())
        })?;

        Self::deposit_event(Event::Removed { id });
        Ok(())
    }

    /// Updates the decision deposit for an existing track with the given Id.
    pub(crate) fn do_set_decision_deposit(
        id: T::TrackId,
        deposit: BalanceOf<T, I>,
    ) -> DispatchResult {
        let (group, track) = id.split();
        Tracks::<T, I>::try_mutate(group, track, |track| -> DispatchResult {
            let track_info = track.as_mut().ok_or(Error::<T, I>::TrackIdNotFound)?;
            track_info.decision_deposit = deposit;
            Ok(())
        })?;

        Ok(())
    }

    /// Updates periods for an existing track with the given Id.
    pub(crate) fn do_set_periods(
        id: T::TrackId,
        prepare: Option<BlockNumberFor<T, I>>,
        decision: Option<BlockNumberFor<T, I>>,
        confirm: Option<BlockNumberFor<T, I>>,
        min_enactment: Option<BlockNumberFor<T, I>>,
    ) -> DispatchResult {
        if let Some(ref period) = decision {
            ensure!(!period.is_zero(), Error::<T, I>::InvalidTrackInfo);
        }
        if let Some(ref period) = confirm {
            ensure!(!period.is_zero(), Error::<T, I>::InvalidTrackInfo);
        }
        let (group, track) = id.split();
        Tracks::<T, I>::try_mutate(group, track, |track| -> DispatchResult {
            let track_info = track.as_mut().ok_or(Error::<T, I>::TrackIdNotFound)?;

            if let Some(period) = prepare {
                track_info.prepare_period = period;
            }
            if let Some(period) = decision {
                track_info.decision_period = period;
            }
            if let Some(period) = confirm {
                track_info.confirm_period = period;
            }
            if let Some(period) = min_enactment {
                track_info.min_enactment_period = period;
            }

            Ok(())
        })?;

        Ok(())
    }

    /// Updates curves for an existing track with the given Id.
    pub(crate) fn do_set_curves(
        id: T::TrackId,
        min_approval: Option<pallet_referenda::Curve>,
        min_support: Option<pallet_referenda::Curve>,
    ) -> DispatchResult {
        let (group, track) = id.split();
        Tracks::<T, I>::try_mutate(group, track, |track| -> DispatchResult {
            let track_info = track.as_mut().ok_or(Error::<T, I>::TrackIdNotFound)?;

            if let Some(curve) = min_approval {
                track_info.min_approval = curve;
            }
            if let Some(curve) = min_support {
                track_info.min_support = curve;
            }

            Ok(())
        })?;

        Ok(())
    }

    /// Updates max_deciding for an existing track with the given Id.
    pub(crate) fn do_set_max_deciding(id: T::TrackId, max_deciding: u32) -> DispatchResult {
        let (group, track) = id.split();
        Tracks::<T, I>::try_mutate(group, track, |track| -> DispatchResult {
            let track_info = track.as_mut().ok_or(Error::<T, I>::TrackIdNotFound)?;
            track_info.max_deciding = max_deciding;
            Ok(())
        })?;
        Ok(())
    }

    /// Removes an entire group and all its sub-tracks. Fails if any track has active referenda.
    /// `max_tracks` is the caller-declared upper bound on sub-tracks for weight purposes.
    pub(crate) fn do_remove_group(group: GroupIdOf<T, I>, max_tracks: u32) -> DispatchResult {
        // Collect sub-tracks with a bounded limit to prevent unbounded allocation.
        let mut sub_tracks =
            alloc::vec::Vec::with_capacity(max_tracks.min(T::MaxTracks::get()) as usize);
        for sub in Tracks::<T, I>::iter_key_prefix(&group) {
            sub_tracks.push(sub);
            ensure!(
                sub_tracks.len() as u32 <= max_tracks,
                Error::<T, I>::TooManyTracks
            );
        }
        ensure!(!sub_tracks.is_empty(), Error::<T, I>::GroupNotFound);

        // Check none have active referenda
        for sub in &sub_tracks {
            let id = T::TrackId::combine(group.clone(), sub.clone());
            ensure!(
                pallet_referenda::DecidingCount::<T, I>::get(id) == 0
                    && pallet_referenda::TrackQueue::<T, I>::get(id).is_empty(),
                Error::<T, I>::CannotRemoveGroup
            );
        }

        // Remove all tracks in the group
        let count = sub_tracks.len() as u32;
        for sub in sub_tracks {
            let id = T::TrackId::combine(group.clone(), sub.clone());
            Tracks::<T, I>::remove(&group, &sub);
            if let Some(origin) = TrackIdToOrigin::<T, I>::take(id) {
                OriginToTrackId::<T, I>::remove(&origin);
            }
            TracksIds::<T, I>::mutate(|ids| {
                ids.remove(&id);
            });
        }

        // Clean up sub-track counter
        NextSubTrackId::<T, I>::remove(&group);

        Self::deposit_event(Event::GroupRemoved {
            group,
            tracks_removed: count,
        });
        Ok(())
    }
}
