use super::*;

use alloc::borrow::Cow;
use frame_support::ensure;
use sp_runtime::{DispatchError, DispatchResult};

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

    /// Inserts a new track into the tracks storage.
    pub(crate) fn do_insert(
        id: T::TrackId,
        info: TrackInfoOf<T, I>,
        origin: OriginOf<T>,
    ) -> DispatchResult {
        let (group, track) = id.split();
        ensure!(
            Self::get_track_info(id).is_none(),
            Error::<T, I>::TrackIdAlreadyExisting
        );

        TracksIds::<T, I>::try_mutate(|ids| ids.try_insert(id))
            .map_err(|_| Error::<T, I>::MaxTracksExceeded)?;
        Tracks::<T, I>::set(group, track, Some(info));
        OriginToTrackId::<T, I>::set(origin.clone(), Some(id));

        Self::deposit_event(Event::Created { id });
        Ok(())
    }

    /// Updates an existing track with the given Id.
    pub(crate) fn do_update(id: T::TrackId, info: TrackInfoOf<T, I>) -> DispatchResult {
        let (group, track) = id.split();
        Tracks::<T, I>::try_mutate(group, track, |track| {
            if track.is_none() {
                return Err(Error::<T, I>::TrackIdNotFound);
            };

            *track = Some(info);

            Ok(())
        })?;

        Ok(())
    }

    pub(crate) fn do_remove(
        id: T::TrackId,
        origin: OriginOf<T>,
    ) -> frame_support::dispatch::DispatchResult {
        let (group, track) = id.split();
        ensure!(
            Tracks::<T, I>::contains_key(&group, &track),
            Error::<T, I>::TrackIdNotFound
        );
        ensure!(
            OriginToTrackId::<T, I>::get(&origin) == Some(id),
            DispatchError::BadOrigin
        );

        Tracks::<T, I>::remove(group, track);
        OriginToTrackId::<T, I>::remove(&origin);
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

            if let Some(curve) = min_approval.clone() {
                track_info.min_approval = curve;
            }
            if let Some(curve) = min_support.clone() {
                track_info.min_support = curve;
            }

            Ok(())
        })?;

        Ok(())
    }
}
