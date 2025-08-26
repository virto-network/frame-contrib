use super::*;

use alloc::borrow::Cow;
use frame_support::ensure;
use sp_runtime::{BoundedVec, DispatchError, DispatchResult};

type OriginOf<T> = <<T as frame_system::Config>::RuntimeOrigin as OriginTrait>::PalletsOrigin;

impl<T: Config<I>, I: 'static> pallet_referenda::TracksInfo<BalanceOf<T, I>, BlockNumberFor<T, I>>
    for Pallet<T, I>
{
    type Id = T::TrackId;
    type RuntimeOrigin = OriginOf<T>;

    fn tracks(
    ) -> impl Iterator<Item = Cow<'static, Track<Self::Id, BalanceOf<T, I>, BlockNumberFor<T, I>>>>
    {
        Tracks::<T, I>::iter().map(|(id, info)| Cow::Owned(Track { id, info }))
    }
    fn track_for(origin: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
        OriginToTrackId::<T, I>::get(origin).ok_or(())
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    /// Inserts a new track into the tracks storage.
    pub(crate) fn do_insert(
        id: T::TrackId,
        info: TrackInfoOf<T, I>,
        origin: OriginOf<T>,
    ) -> DispatchResult {
        ensure!(
            Tracks::<T, I>::get(id).is_none(),
            Error::<T, I>::TrackIdAlreadyExisting
        );

        TracksIds::<T, I>::try_append(id).map_err(|_| Error::<T, I>::MaxTracksExceeded)?;
        Tracks::<T, I>::set(id, Some(info));
        OriginToTrackId::<T, I>::set(origin.clone(), Some(id));

        Self::deposit_event(Event::Created { id });
        Ok(())
    }

    /// Updates an existing track with the given Id.
    pub(crate) fn do_update(id: T::TrackId, info: TrackInfoOf<T, I>) -> DispatchResult {
        Tracks::<T, I>::try_mutate(id, |track| {
            if track.is_none() {
                return Err(Error::<T, I>::TrackIdNotFound);
            };

            *track = Some(info);

            Ok(())
        })?;

        Self::deposit_event(Event::Updated { id });
        Ok(())
    }

    pub(crate) fn do_remove(
        id: T::TrackId,
        origin: OriginOf<T>,
    ) -> frame_support::dispatch::DispatchResult {
        ensure!(
            Tracks::<T, I>::contains_key(id),
            Error::<T, I>::TrackIdNotFound
        );
        ensure!(
            OriginToTrackId::<T, I>::get(&origin) == Some(id),
            DispatchError::BadOrigin
        );

        Tracks::<T, I>::remove(id);
        OriginToTrackId::<T, I>::remove(&origin);
        TracksIds::<T, I>::try_mutate(|tracks_ids| {
            let new_tracks_ids = tracks_ids
                .clone()
                .into_iter()
                .filter(|i| i != &id)
                .collect();
            *tracks_ids = BoundedVec::<_, _>::truncate_from(new_tracks_ids);

            Ok::<(), DispatchError>(())
        })?;

        Self::deposit_event(Event::Removed { id });
        Ok(())
    }
}
