#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::*;

mod types;
pub use types::{Proposal, ProposalStatus, Vote, VoteDecision};

pub type ProposalId = u32;

#[frame_support::pallet]
pub mod pallet {
	use core::cmp::Ordering;

	use frame_support::{
		ensure,
		pallet_prelude::*,
		traits::{Currency, LockableCurrency, ReservableCurrency},
		Blake2_128Concat,
	};
	use frame_system::{pallet_prelude::{OriginFor, *}};

	use crate::{Proposal, ProposalId, ProposalStatus, Vote, VoteDecision, WeightInfo};

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Type to access the Balances Pallet.
		type Currency: Currency<Self::AccountId>
			+ ReservableCurrency<Self::AccountId>
			+ LockableCurrency<Self::AccountId>;

		///Period of time at the end of a proposal during which votes cannot be reduced or
		/// cancelled.
		type VoteRemovalThreshold: Get<u32>;

		///The limit of voter that can be registered to vote in the pallet.
		type MaxVoters: Get<u32>;

		///The limit of points an individual vote can have.
		type VoteLimit: Get<u32>;

		///Weight Information
		type WeightInfo: WeightInfo;
	}

	///Contains all users registered by the root that are eligible to vote.
	#[pallet::storage]
	pub type RegisteredVoters<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	///Current amount of registered voters
	#[pallet::storage]
	pub type AmountVoters<T: Config> = StorageValue<_, u32>;

	///Holds user-made proposals, identified by a ProposalId, and the actual proposal data.
	#[pallet::storage]
	pub type Proposals<T: Config> = StorageMap<_, Blake2_128Concat, ProposalId, Proposal<T>>;

	///Holds the votes made by registered voters for a specific proposal. The first key is the
	/// T::AccountId of the voter, and the second key is the ProposalId.
	#[pallet::storage]
	pub type Votes<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, ProposalId, Vote>;

	///Holds the counter used to increase the ProposalId of proposals.
	#[pallet::storage]
	pub type ProposalCounter<T: Config> = StorageValue<_, ProposalId>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		///New voter 'T::AccountId' registered by root into the RegisteredVoters list.
		VoterRegistered { who: T::AccountId },
		///A user submitted a new proposal
		ProposalSubmitted { proposal_id: ProposalId, who: T::AccountId },
		///A registered voter casted a vote for a specific proposal
		VoteCasted { proposal_id: ProposalId, who: T::AccountId },
		///Registered voter updated their vote for Proposal ID from 'previous' to 'new' decision.
		VoteUpdated {
			proposal_id: ProposalId,
			who: T::AccountId,
			previous: VoteDecision,
			new: VoteDecision,
		},
		///A voter canceled his vote for an ongoing proposal
		VoteCanceled { proposal_id: ProposalId, who: T::AccountId },
		///Proposal ended and result is defined
		ProposalEnded { proposal_id: ProposalId, status: ProposalStatus },
		///Proposal end time updated for Proposal ID: 'ProposalId' with new end block as
		/// 'BlockNumberFor<T>'
		ProposalUpdated { proposal_id: ProposalId, end_block: BlockNumberFor<T> },
		///Proposal canceled by the proposer
		ProposalCanceled { proposal_id: ProposalId },
		///User unlocked balance of a specific proposal
		BalanceUnlocked { proposal_id: ProposalId, who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		///Voter already registered
		AlreadyRegistered,
		///Voter is not registered to cast vote
		VoterIsNotRegistered,
		///Maximum registered voters limit has been reached.
		MaxVotersLimitReached,
		///Voter's vote for the proposal is already registered.
		VoteAlreadyCasted,
		///Vote not found for user and proposal
		VoteNotFound,
		///Vote amount exceeds the defined limit.
		VoteAmountLimit,
		///Invalid vote amount. The number of points exceeds accepted limits.
		InvalidVoteAmount,
		///The received amount of votes to update is invalid.
		InvalidUpdateAmount,
		///Invalid time period. Received block number is equal or less than current block number.
		TimePeriodToLow,
		///The proposal counter reached overflow limit
		ProposalIdToHigh,
		///Proposal not found. The requested proposal does not exist.
		ProposalNotFound,
		///Unauthorized user. The user lacks permission to execute the extrinsic.
		Unauthorized,
		///The proposal has already ended and cannot be modified.
		ProposalAlreadyEnded,
		///The balance for the current vote has already been released.
		BalanceAlreadyUnocked,
		///The proposal's remaining time has exceeded the limit for reducing or cancelling votes.
		PassedRemovalThreshold,
		///The proposal is ongoing, so the balance cannot be released.
		ProposalInProgress,
		///Overflow when performing an operation
		Overflow,
	}

	#[pallet::call(weight(<T as Config>::WeightInfo))]
	impl<T: Config> Pallet<T> {
		/// Registers a voter into the list of registered voters
		/// if they have not already been registered
		/// or if the maximum number of voters has not been reached.
		///
		/// Origin must be root user.
		#[pallet::call_index(0)]
		pub fn register_voter(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(!Self::is_registered(&who), Error::<T>::AlreadyRegistered);

			let amount_voters: u32 = <AmountVoters<T>>::try_get().unwrap_or_default();
			ensure!(amount_voters < T::MaxVoters::get(), Error::<T>::MaxVotersLimitReached);

			//Register voter and increase voter counter
			<RegisteredVoters<T>>::insert(who.clone(), ());
			<AmountVoters<T>>::put(amount_voters.saturating_add(1));

			Self::deposit_event(Event::VoterRegistered { who });
			Ok(())
		}

		/// Creates a new proposal for voting.
		/// The proposal contains a hashed description and a voting time limit in blocks.
		///
		/// Only registered voters can create proposals.
		#[pallet::call_index(1)]
		pub fn make_proposal(
			origin: OriginFor<T>,
			description: T::Hash,
			time_period: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Self::is_registered(&who), Error::<T>::VoterIsNotRegistered);

			let current_block_number = <frame_system::Pallet<T>>::block_number();
			ensure!(time_period > current_block_number, Error::<T>::TimePeriodToLow);

			let mut proposal_id: ProposalId = ProposalCounter::<T>::get().unwrap_or_default();
			ensure!(proposal_id.checked_add(1).is_some(), Error::<T>::ProposalIdToHigh);
			proposal_id = proposal_id + 1;

			let new_proposal =
				Proposal::<T>::new(proposal_id, who.clone(), description, time_period);

			<Proposals<T>>::insert(proposal_id, new_proposal);
			<ProposalCounter<T>>::put(proposal_id);
			Self::deposit_event(Event::ProposalSubmitted { proposal_id, who });

			Ok(())
		}

		/// Extends the voting period of a proposal by increasing its time limit in blocks.
		///
		/// Only the user who created the proposal can call this extrinsic.
		#[pallet::call_index(2)]
		#[pallet::weight(
			T::WeightInfo::increase_proposal_time(ProposalCounter::<T>::get().unwrap_or_default())
		)]
		pub fn increase_proposal_time(
			origin: OriginFor<T>,
			proposal_id: ProposalId,
			new_time_period: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Self::is_registered(&who), Error::<T>::VoterIsNotRegistered);

			let proposal = Self::get_proposal(&proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
			ensure!(proposal.proposer == who, Error::<T>::Unauthorized);

			let current_block_number = <frame_system::Pallet<T>>::block_number();
			ensure!(new_time_period > proposal.time_period, Error::<T>::TimePeriodToLow);
			ensure!(new_time_period > current_block_number, Error::<T>::TimePeriodToLow);

			<Proposals<T>>::mutate(proposal_id, |proposal| {
				if let Some(p) = proposal.as_mut() {
					p.time_period = new_time_period
				}
			});

			Self::deposit_event(Event::ProposalUpdated { proposal_id, end_block: new_time_period });

			Ok(())
		}

		/// Cancel a proposal if it hasn't ended yet
		///
		/// The proposal can only be cancelled by the user who created it.
		#[pallet::call_index(3)]
		#[pallet::weight(
			T::WeightInfo::increase_proposal_time(ProposalCounter::<T>::get().unwrap_or_default())
		)]
		pub fn cancel_proposal(origin: OriginFor<T>, proposal_id: ProposalId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let proposal = Self::get_proposal(&proposal_id).ok_or(Error::<T>::ProposalNotFound)?;

			ensure!(proposal.proposer == who, Error::<T>::Unauthorized);
			ensure!(
				proposal.status == ProposalStatus::InProgress,
				Error::<T>::ProposalAlreadyEnded
			);

			let current_block_number = <frame_system::Pallet<T>>::block_number();
			ensure!(proposal.time_period > current_block_number, Error::<T>::TimePeriodToLow);

			<Proposals<T>>::mutate(proposal_id, |proposal| {
				if let Some(p) = proposal.as_mut() {
					p.status = ProposalStatus::Canceled
				}
			});
			Self::deposit_event(Event::ProposalCanceled { proposal_id });

			Ok(())
		}

		/// Allows a registered voter to vote on a proposal if it's still ongoing. The vote
		/// increases the ayes or nays votes of the proposal based on the number of vote points.

		/// To vote, the user must reserve the balance of their account, equal to the square
		/// of the number of votes they want to cast.
		// The number of votes must be greater than zero and less than the VoteLimit.
		#[pallet::call_index(4)]
		#[pallet::weight(0)]
		pub fn vote(
			origin: OriginFor<T>,
			proposal_id: ProposalId,
			vote_decision: VoteDecision,
		) -> DispatchResult {
			//Verify sender is part of register voters
			let who: T::AccountId = ensure_signed(origin)?;
			ensure!(Self::is_registered(&who), Error::<T>::VoterIsNotRegistered);

			let proposal = Self::get_proposal(&proposal_id).ok_or(Error::<T>::ProposalNotFound)?;

			//Check that propossal is not passed removal_treshold
			let current_block_number = <frame_system::Pallet<T>>::block_number();
			ensure!(
				proposal.time_period > current_block_number &&
					proposal.status == ProposalStatus::InProgress,
				Error::<T>::ProposalAlreadyEnded
			);

			//Verify if voter already casted vote
			ensure!(!Self::vote_casted(&who, &proposal_id), Error::<T>::VoteAlreadyCasted);

			let vote_amount = match vote_decision {
				VoteDecision::Aye(v) => v,
				VoteDecision::Nay(v) => v,
			};

			ensure!(vote_amount > 0, Error::<T>::InvalidVoteAmount);
			ensure!(vote_amount <= T::VoteLimit::get(), Error::<T>::VoteAmountLimit);

			//Reserve balance corresponding to vote amount^2.
			let amount_to_reserve: u32 =
				(vote_amount).checked_pow(2).ok_or(Error::<T>::Overflow)?;
			T::Currency::reserve(&who, amount_to_reserve.into())?;

			let vote = Vote { vote_decision: vote_decision.clone(), locked: true };

			//Insert vote and update proposals
			<Votes<T>>::insert(who.clone(), proposal_id, vote.clone());

			<Proposals<T>>::mutate(proposal_id, |proposal| {
				if let Some(p) = proposal.as_mut() {
					match vote_decision {
						VoteDecision::Aye(v) => p.ayes += v,
						VoteDecision::Nay(v) => p.nays += v,
					}
				}
			});

			Self::deposit_event(Event::VoteCasted { proposal_id, who });
			Ok(())
		}

		/// Updates the vote of a voter in a proposal with a new amount of points and the ability
		/// to switch between aye and nay.
		///
		/// The function performs several checks and updates the balance of the user and the
		/// vote count of the proposal if the new vote differs from the original.
		///
		/// - Check that the proposal is still in progress and has not passed the removal threshold.
		///   If the threshold is surppased the voter cant reduce the amount of votes.
		/// - Calculate the new amount of vote points and update the aye or nay count accordingly.
		/// - Reserve or unreserve the user's balance based on the comparison between the current
		///   and new vote amounts.
		/// - Update the vote record in storage and emit an event for the vote update.
		#[pallet::call_index(5)]
		#[pallet::weight(0)]
		pub fn update_vote(
			origin: OriginFor<T>,
			proposal_id: ProposalId,
			new_vote_decision: VoteDecision,
		) -> DispatchResult {
			//Verify sender is part of register voters and vote exists
			let who: T::AccountId = ensure_signed(origin)?;
			ensure!(Self::is_registered(&who), Error::<T>::VoterIsNotRegistered);

			let mut proposal =
				Self::get_proposal(&proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
			//Check that propossal is not passed removal_treshold
			let current_block_number = <frame_system::Pallet<T>>::block_number();
			ensure!(
				proposal.time_period > current_block_number &&
					proposal.status == ProposalStatus::InProgress,
				Error::<T>::ProposalAlreadyEnded
			);

			//Get vote and verify if it exists
			let current_vote =
				<Votes<T>>::try_get(&who, &proposal_id).ok().ok_or(Error::<T>::VoteNotFound)?;

			let current_amount: u32 = match current_vote.vote_decision {
				VoteDecision::Aye(v) => {
					proposal.ayes = proposal.ayes.saturating_sub(v);
					v
				},
				VoteDecision::Nay(v) => {
					proposal.nays = proposal.nays.saturating_sub(v);
					v
				},
			};

			let new_amount = match new_vote_decision {
				VoteDecision::Aye(v) => {
					proposal.ayes += v;
					v
				},
				VoteDecision::Nay(v) => {
					proposal.nays += v;
					v
				},
			};
			if new_amount.cmp(&current_amount) == Ordering::Less {
				//Check threshold
				ensure!(
					!Self::passed_removal_threshold(&proposal.time_period),
					Error::<T>::PassedRemovalThreshold
				);
			}

			ensure!(new_amount != 0, Error::<T>::InvalidUpdateAmount);
			ensure!(new_amount <= T::VoteLimit::get(), Error::<T>::VoteAmountLimit);

			let current_amount_pow: u32 =
				current_amount.checked_pow(2).ok_or(Error::<T>::Overflow)?;
			let new_amount_pow: u32 = new_amount.checked_pow(2).ok_or(Error::<T>::Overflow)?;

			//Modify reserved amount
			match new_amount.cmp(&current_amount) {
				Ordering::Greater => {
					T::Currency::reserve(&who, (new_amount_pow - current_amount_pow).into())?;
				},
				Ordering::Less => {
					T::Currency::unreserve(&who, (current_amount_pow - new_amount_pow).into());
				},
				_ => (),
			};

			let new_vote = Vote { vote_decision: new_vote_decision, locked: true };

			<Votes<T>>::insert(who.clone(), proposal_id, new_vote.clone());
			<Proposals<T>>::insert(proposal_id, proposal);
			Self::deposit_event(Event::VoteUpdated {
				proposal_id,
				who,
				previous: current_vote.vote_decision,
				new: new_vote.vote_decision,
			});

			Ok(())
		}

		///Enables a voter to revoke their vote for a proposal, provided that the RemovalThreshold
		///has not been surpassed.
		///
		/// It then updates the count of votes in favor (ayes) or against (nays) accordingly.
		///
		/// Returns the reserved balance to the voter
		#[pallet::call_index(9)]
		#[pallet::weight(0)]
		pub fn cancel_vote(origin: OriginFor<T>, proposal_id: ProposalId) -> DispatchResult {
			let who: T::AccountId = ensure_signed(origin)?;
			//Allows to calculate treshold

			let mut proposal =
				Self::get_proposal(&proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
			let vote: Vote = <Votes<T>>::try_get(who.clone(), proposal_id)
				.ok()
				.ok_or(Error::<T>::VoteNotFound)?;
			let current_block_number = <frame_system::Pallet<T>>::block_number();

			ensure!(
				proposal.time_period >= current_block_number &&
					proposal.status == ProposalStatus::InProgress,
				Error::<T>::ProposalAlreadyEnded
			);

			//Check that propossal is not passed removal_treshold
			ensure!(
				!Self::passed_removal_threshold(&proposal.time_period),
				Error::<T>::PassedRemovalThreshold
			);

			match vote.vote_decision {
				VoteDecision::Aye(v) => proposal.ayes = proposal.ayes.saturating_sub(v),
				VoteDecision::Nay(v) => proposal.nays = proposal.nays.saturating_sub(v),
			}

			<Proposals<T>>::insert(proposal_id, proposal);
			<Votes<T>>::remove(who.clone(), proposal_id);

			let vote_amount = match vote.vote_decision {
				VoteDecision::Aye(v) => v,
				VoteDecision::Nay(v) => v,
			};

			//unreserve balance corresponding to the vote (amount^2).
			let amount_to_unreserve: u32 =
				(vote_amount).checked_pow(2).ok_or(Error::<T>::Overflow)?;
			T::Currency::unreserve(&who, amount_to_unreserve.into());

			Self::deposit_event(Event::VoteCanceled { proposal_id, who });

			Ok(())
		}

		/// Finishes a proposal by calculating the result based on the number of ayes and nays.
		///
		/// The proposal can only be finished if the time limit (in blocks) has been
		/// exceeded and the status of the proposal is 'In Progress'.
		///
		/// This extrinsic can be called by any registered voter.
		#[pallet::call_index(7)]
		#[pallet::weight(0)]
		pub fn finish_proposal(origin: OriginFor<T>, proposal_id: ProposalId) -> DispatchResult {
			//Verify sender is part of register voters and vote exists
			let who: T::AccountId = ensure_signed(origin)?;
			ensure!(Self::is_registered(&who), Error::<T>::VoterIsNotRegistered);

			let mut proposal =
				Self::get_proposal(&proposal_id).ok_or(Error::<T>::ProposalNotFound)?;

			let current_block_number = <frame_system::Pallet<T>>::block_number();
			ensure!(
				proposal.time_period < current_block_number &&
					proposal.status == ProposalStatus::InProgress,
				Error::<T>::ProposalAlreadyEnded
			);

			let voting_result: ProposalStatus = match proposal.ayes.cmp(&proposal.nays) {
				Ordering::Less => ProposalStatus::Rejected,
				Ordering::Greater => ProposalStatus::Passed,
				Ordering::Equal => ProposalStatus::Tied,
			};

			proposal.status = voting_result.clone();

			<Proposals<T>>::insert(proposal_id, proposal);
			Self::deposit_event(Event::ProposalEnded { proposal_id, status: voting_result });
			Ok(())
		}

		///Unlocks the locked balance of a voter for a finished proposal.
		///
		///This extrinsic can be called by the voter.
		/// Returns an error if the proposal is still in progress or if the balance
		/// has already been unlocked.
		#[pallet::call_index(8)]
		#[pallet::weight(0)]
		pub fn unlock_balance(origin: OriginFor<T>, proposal_id: ProposalId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let proposal: Proposal<T> =
				Self::get_proposal(&proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
			ensure!(proposal.status != ProposalStatus::InProgress, Error::<T>::ProposalInProgress);

			let mut vote: Vote = <Votes<T>>::try_get(who.clone(), proposal_id)
				.ok()
				.ok_or(Error::<T>::VoteNotFound)?;
			ensure!(vote.locked, Error::<T>::BalanceAlreadyUnocked);
			vote.locked = false;
			<Votes<T>>::insert(who.clone(), proposal_id, vote.clone());

			let vote_amount = match vote.vote_decision {
				VoteDecision::Aye(v) => v,
				VoteDecision::Nay(v) => v,
			};

			//unreserve balance corresponding to the vote (amount^2).
			let amount_to_unreserve: u32 =
				(vote_amount).checked_pow(2).ok_or(Error::<T>::Overflow)?;
			T::Currency::unreserve(&who, amount_to_unreserve.into());

			Self::deposit_event(Event::BalanceUnlocked { proposal_id, who });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn is_registered(who: &T::AccountId) -> bool {
			RegisteredVoters::<T>::contains_key(who)
		}

		pub fn proposal_exists(proposal_id: ProposalId) -> bool {
			Proposals::<T>::contains_key(proposal_id)
		}
		pub fn get_proposal_counter() -> ProposalId {
			ProposalCounter::<T>::get().unwrap_or_default()
		}
		pub fn get_proposal(proposal_id: &ProposalId) -> Option<Proposal<T>> {
			<Proposals<T>>::get(proposal_id)
		}
		pub fn vote_casted(who: &T::AccountId, proposal_id: &ProposalId) -> bool {
			if <Votes<T>>::try_get(who, proposal_id).is_err() {
				return false
			};
			true
		}
		pub fn passed_removal_threshold(end_time_period: &BlockNumberFor<T>) -> bool {
			let current_block_number = <frame_system::Pallet<T>>::block_number();

			let difference = *end_time_period - current_block_number;
			difference < T::VoteRemovalThreshold::get().into()
		}
	}
}
