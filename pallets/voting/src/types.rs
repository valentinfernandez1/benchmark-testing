use codec::{Decode, Encode, MaxEncodedLen};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;

use crate::{Config, ProposalId};

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone)]
#[scale_info(skip_type_params(T))]
pub struct Proposal<T: Config> {
	pub id: ProposalId,
	pub proposer: T::AccountId,
	pub text: T::Hash,
	pub time_period: BlockNumberFor<T>,
	pub status: ProposalStatus,
	pub ayes: u32,
	pub nays: u32,
}

impl<T: Config> Proposal<T> {
	pub fn new(
		id: ProposalId,
		proposer: T::AccountId,
		text: T::Hash,
		time_period: BlockNumberFor<T>,
	) -> Self {
		Proposal {
			id,
			proposer,
			text,
			time_period,
			status: ProposalStatus::InProgress,
			ayes: 0,
			nays: 0,
		}
	}
}

#[derive(Encode, Debug, Decode, Clone, TypeInfo, MaxEncodedLen, Eq, PartialEq)]
pub struct Vote {
	pub vote_decision: VoteDecision,
	pub locked: bool,
}

#[derive(Encode, Debug, Decode, Clone, TypeInfo, MaxEncodedLen, Eq, PartialEq)]
pub enum VoteDecision {
	Aye(u32),
	Nay(u32),
}

#[derive(Encode, Debug, Decode, TypeInfo, MaxEncodedLen, Clone, Eq, PartialEq)]
#[scale_info(skip_type_params(T))]
pub enum ProposalStatus {
	InProgress,
	Canceled,
	Passed,
	Rejected,
	Tied,
}
