#![cfg(feature = "runtime-benchmarks")]
use super::*;

#[allow(unused)]
use crate::Pallet as Voting;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

const SEED: u32 = 0;

#[benchmarks(
	where 
	<<<T as frame_system::Config>::Block as frame_support::sp_runtime::traits::Block>::Header as frame_support::sp_runtime::traits::Header>::Number: From<u32>,
	T: frame_system::Config<Hash = H256>
)]
pub mod benchmarks {
	
	use sp_core::H256;
	use frame_support::traits::Currency;
	use super::*;

	fn get_registered_proposer<T: Config>() -> T::AccountId {
		let proposer: T::AccountId = account("proposer", 0, SEED);
		let _ = Voting::<T>::register_voter(RawOrigin::Root.into(), proposer.clone());
	
		proposer
	}

	
	#[benchmark]
	fn register_voter() {
		//setup
		let voter: T::AccountId = account("recipient", 0, SEED);
		
		#[extrinsic_call]
		_(RawOrigin::Root, voter.clone());
		
		//verify
		assert!(Voting::<T>::is_registered(&voter));
	}
	
	#[benchmark]
	fn make_proposal() {
		let description = H256([0;32]);
		let time_period: u32 = 100000;
		let proposer = get_registered_proposer::<T>();

		#[extrinsic_call]
		_(RawOrigin::Signed(proposer), description, time_period.into());

		//verify
		let counter = Voting::<T>::get_proposal_counter();
		assert!(Voting::<T>::proposal_exists(counter));
	}

	#[benchmark]
	fn increase_proposal_time(x: Linear<1, 10_000>){
		//setup
		let proposer = get_registered_proposer::<T>();
		for i in 0..x {
			Proposals::<T>::insert(
				i.clone(),
				Proposal::<T>::new(i, proposer.clone(), H256([0;32]), 100_000u32.into()));
		}

		let id = x-1;
		let time_period: u32 = 200000;

		#[extrinsic_call]
		_(RawOrigin::Signed(proposer), id.clone(), time_period.into());

		//verify
		let updated_proposal = Voting::<T>::get_proposal(&id);
		assert_eq!(updated_proposal.unwrap().time_period, time_period.into());
	}

	#[benchmark]
	fn cancel_proposal(x: Linear<1, 10_000>){
		//setup
		let proposer = get_registered_proposer::<T>();
		for i in 0..x {
			Proposals::<T>::insert(
				i.clone(),
				Proposal::<T>::new(i, proposer.clone(), H256([0;32]), 100_000u32.into()));
		}

		let id = x-1;
		#[extrinsic_call]
		_(RawOrigin::Signed(proposer), id.clone());
	
		//verify
		assert_eq!(
			Voting::<T>::get_proposal(&id).unwrap().status, 
			ProposalStatus::Canceled
		);
	}

	#[benchmark]
	fn vote(){
		//setup
		let voter_proposer = get_registered_proposer::<T>();
		Proposals::<T>::insert(1, Proposal::<T>::new(1, voter_proposer.clone(), H256([0;32]), 100_000u32.into()));
		let _ = T::Currency::make_free_balance_be(&voter_proposer, 100u32.into());

		#[extrinsic_call]
		_(RawOrigin::Signed(voter_proposer.clone()), 1, VoteDecision::Aye(1));

		//verify
		assert!(Voting::<T>::vote_casted(&voter_proposer, &1));
	}

	impl_benchmark_test_suite!(Voting, crate::mock::new_test_ext(), crate::mock::Test,);
}

