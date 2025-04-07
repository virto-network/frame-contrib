use super::*;

use frame_support::ensure;
use sp_runtime::{BoundedVec, DispatchError, DispatchResult};

impl<T: Config<I>, I> pallet_referenda::TracksInfo<BalanceOf<T, I>, BlockNumberFor<T>>
for Pallet<T, I>
{
	type Id = T::TrackId;
	type RuntimeOrigin = <T::RuntimeOrigin as OriginTrait>::PalletsOrigin;
	type TracksIter = TracksIter<T, I>;

	fn tracks() -> Self::TracksIter {
		Tracks::<T, I>::iter().map(|(id, info)| Cow::Owned(Track { id, info }))
	}
	fn track_for(origin: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
		OriginToTrackId::<T, I>::get(origin).ok_or(())
	}
	fn tracks_ids() -> Vec<Self::Id> {
		TracksIds::<T, I>::get().into_inner()
	}
	fn info(id: Self::Id) -> Option<Cow<'static, TrackInfoOf<T, I>>> {
		Tracks::<T, I>::get(id).map(Cow::Owned)
	}
}

impl<T: Config<I>, I: 'static> Get<Vec<TrackOf<T, I>>> for crate::Pallet<T, I> {
	fn get() -> Vec<TrackOf<T, I>> {
		// expensive but it doesn't seem to be used anywhere
		<Pallet<T, I> as pallet_referenda::TracksInfo<BalanceOf<T, I>, BlockNumberFor<T>>>::tracks()
			.map(|t| t.into_owned())
			.collect()
	}
}

impl<T: Config<I>, I> fc_traits_tracks::MutateTracks<BalanceOf<T, I>, BlockNumberFor<T>>
for Pallet<T, I>
{
	/// Inserts a new track into the tracks storage.
	fn insert(
		id: Self::Id,
		info: TrackInfoOf<T, I>,
		origin: Self::RuntimeOrigin,
	) -> DispatchResult {
		ensure!(
            Tracks::<T, I>::get(id) == None,
            Error::<T, I>::TrackIdAlreadyExisting
        );

		TracksIds::<T, I>::try_append(id).map_err(|_| Error::<T, I>::MaxTracksExceeded)?;
		Tracks::<T, I>::set(id, Some(info));
		OriginToTrackId::<T, I>::set(origin.clone(), Some(id));

		Self::deposit_event(Event::Created { id });
		Ok(())
	}

	/// Updates an existing track with the given Id.
	fn update(id: Self::Id, info: TrackInfoOf<T, I>) -> DispatchResult {
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

	fn remove(
		id: Self::Id,
		origin: Self::RuntimeOrigin,
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
		Ok(().into())
	}
}
