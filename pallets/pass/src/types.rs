use frame_system::Config as SystemConfig;

pub type AccountIdOf<T> = <T as SystemConfig>::AccountId;
