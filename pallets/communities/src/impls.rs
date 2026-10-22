use frame_support::traits::VoteTally;
use sp_runtime::{traits::Convert, Perbill};

use crate::{
    types::{CommunityIdOf, Tally, VoteWeight},
    Config,
};

/// A tally is of the class of its poll (a referenda track), and measures its support against the
/// community that class belongs to ([`Config::CommunityOfClass`]).
impl<T: Config, Class> VoteTally<VoteWeight, Class> for Tally<T>
where
    T::CommunityOfClass: Convert<Class, Option<CommunityIdOf<T>>>,
{
    fn new(_class: Class) -> Self {
        Self::default()
    }

    fn ayes(&self, _class: Class) -> VoteWeight {
        self.ayes
    }

    fn support(&self, class: Class) -> sp_runtime::Perbill {
        match T::CommunityOfClass::convert(class) {
            Some(community_id) => {
                Perbill::from_rational(self.bare_ayes, Self::max_support(community_id))
            }
            // A class that belongs to no community has no electorate to measure support against.
            None => Perbill::zero(),
        }
    }

    fn approval(&self, _class: Class) -> sp_runtime::Perbill {
        Perbill::from_rational(self.ayes, 1.max(self.ayes + self.nays))
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn unanimity(class: Class) -> Self {
        let max_support = Self::max_support_of_class(class);
        Self {
            ayes: max_support,
            bare_ayes: max_support,
            nays: 0,
            ..Default::default()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn rejection(class: Class) -> Self {
        Self {
            ayes: 0,
            bare_ayes: 0,
            nays: Self::max_support_of_class(class),
            ..Default::default()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn from_requirements(support: Perbill, approval: Perbill, class: Class) -> Self {
        let max_support = Self::max_support_of_class(class);
        let approval_weight = approval * max_support;
        let rejection_weight = (Perbill::from_percent(100) - approval) * max_support;
        let support_weight = support * max_support;

        Self {
            ayes: approval_weight,
            nays: rejection_weight,
            bare_ayes: support_weight,
            ..Default::default()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn setup(_class: Class, _granularity: Perbill) {}
}

#[cfg(feature = "runtime-benchmarks")]
impl<T: Config> Tally<T> {
    /// The maximum support of the community a class belongs to; `0` if it belongs to none.
    fn max_support_of_class<Class>(class: Class) -> VoteWeight
    where
        T::CommunityOfClass: Convert<Class, Option<CommunityIdOf<T>>>,
    {
        T::CommunityOfClass::convert(class)
            .map(Self::max_support)
            .unwrap_or_default()
    }
}
