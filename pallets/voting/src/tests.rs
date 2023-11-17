use crate::{mock::*, Error, Event, Proposal, ProposalStatus, VoteDecision};
use frame_support::{assert_noop, assert_ok, traits::Currency};

mod register_voter {
	use super::*;

	#[test]
	fn voter_registration() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);

			//Register new voter
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 2));
			assert!(Voting::is_registered(&2));
			assert!(System::events().len() == 1);
			System::assert_has_event(Event::VoterRegistered { who: 2 }.into());

			//Try to re-register the same voter;
			assert_noop!(
				Voting::register_voter(RuntimeOrigin::root(), 2),
				Error::<Test>::AlreadyRegistered
			);
		});
	}

	#[test]
	fn register_invalid_origin() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Voting::register_voter(RuntimeOrigin::signed(1), 2),
				sp_runtime::DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn reached_max_voters() {
		new_test_ext().execute_with(|| {
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 2));
			MaxVoters::set(1);
			assert_noop!(
				Voting::register_voter(RuntimeOrigin::root(), 3),
				Error::<Test>::MaxVotersLimitReached
			);
		});
	}
}

mod create_proposal {
	use super::*;

	#[test]
	fn make_proposal() {
		new_test_ext().execute_with(|| {
			System::set_block_number(82);
			let initial_proposal_id = Voting::get_proposal_counter();
			let new_proposal_id = initial_proposal_id + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));

			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));
			assert!(Voting::proposal_exists(new_proposal_id));

			System::assert_has_event(
				Event::ProposalSubmitted { proposal_id: new_proposal_id, who: 1 }.into(),
			);

			assert_eq!(initial_proposal_id + 1, Voting::get_proposal_counter());
		});
	}

	#[test]
	fn proposal_time_low() {
		new_test_ext().execute_with(|| {
			System::set_block_number(82);
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));

			assert_noop!(
				Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 80),
				Error::<Test>::TimePeriodToLow
			);
		});
	}

	#[test]
	fn proposer_not_registeredd() {
		new_test_ext().execute_with(|| {
			System::set_block_number(82);

			assert_noop!(
				Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90),
				Error::<Test>::VoterIsNotRegistered
			);
		});
	}
}

mod increase_proposal_time {
	use super::*;

	#[test]
	fn update_proposal() {
		new_test_ext().execute_with(|| {
			System::set_block_number(30);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));

			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));
			assert_ok!(Voting::increase_proposal_time(RuntimeOrigin::signed(1), proposal_id, 95));

			System::assert_has_event(Event::ProposalUpdated { proposal_id, end_block: 95 }.into());

			let updated_proposal: Proposal<Test> = Voting::get_proposal(&proposal_id).unwrap();
			assert_eq!(updated_proposal.time_period, 95)
		});
	}

	#[test]
	fn proposer_not_registered() {
		new_test_ext().execute_with(|| {
			System::set_block_number(30);

			assert_noop!(
				Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90),
				Error::<Test>::VoterIsNotRegistered
			);
		});
	}

	#[test]
	fn update_proposal_invalid() {
		new_test_ext().execute_with(|| {
			System::set_block_number(30);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));

			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));
			assert_noop!(
				Voting::increase_proposal_time(RuntimeOrigin::signed(1), proposal_id, 75),
				Error::<Test>::TimePeriodToLow
			);
		});
	}

	#[test]
	fn invalid_proposer_update() {
		new_test_ext().execute_with(|| {
			System::set_block_number(30);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 2));

			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));
			assert_noop!(
				Voting::increase_proposal_time(RuntimeOrigin::signed(2), proposal_id, 95),
				Error::<Test>::Unauthorized
			);
		});
	}
}

mod cancel_proposal {
	use super::*;

	#[test]
	fn proposal_canceled() {
		new_test_ext().execute_with(|| {
			System::set_block_number(30);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));

			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));
			assert_ok!(Voting::cancel_proposal(RuntimeOrigin::signed(1), proposal_id));
			System::assert_has_event(Event::ProposalCanceled { proposal_id }.into());

			let updated_proposal: Proposal<Test> = Voting::get_proposal(&proposal_id).unwrap();
			assert_eq!(updated_proposal.status, ProposalStatus::Canceled);
		});
	}

	#[test]
	fn proposal_cant_be_canceled() {
		new_test_ext().execute_with(|| {
			System::set_block_number(30);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));

			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));

			System::set_block_number(100);

			assert_noop!(
				Voting::cancel_proposal(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::TimePeriodToLow
			);
		});
	}
}

mod vote {
	use super::*;

	#[test]
	fn cast_valid_votes() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 2));
			let initial_balance: u32 = 25;
			Balances::make_free_balance_be(&1, initial_balance.into());
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));

			//Vote in favor and verify that the functions excecutes properly and the event is
			// created
			let vote_amount: u32 = 2;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));
			System::assert_has_event(Event::VoteCasted { proposal_id, who: 1 }.into());

			//Check that the reserved amount from the user is (amount of votes^2)
			let user_balance = Balances::free_balance(&1);
			assert_eq!(initial_balance, user_balance as u32 + vote_amount.pow(2));

			//Check that the vote is in storage and the proposal updated properly
			assert!(Voting::vote_casted(&1, &proposal_id));
			let updated_proposal: Proposal<Test> = Voting::get_proposal(&proposal_id).unwrap();
			assert_eq!(updated_proposal.ayes, vote_amount);

			//Vote nay and verify that the changes are correct in storage
			Balances::make_free_balance_be(&2, 25u32.into());
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(2),
				proposal_id,
				VoteDecision::Nay(vote_amount)
			));
			System::assert_has_event(Event::VoteCasted { proposal_id, who: 2 }.into());
			assert!(Voting::vote_casted(&2, &proposal_id));
			let updated_proposal: Proposal<Test> = Voting::get_proposal(&proposal_id).unwrap();
			assert_eq!(updated_proposal.nays, vote_amount);
		});
	}

	#[test]
	fn voter_not_registered() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));

			assert_noop!(
				Voting::vote(RuntimeOrigin::signed(2), proposal_id, VoteDecision::Aye(1)),
				Error::<Test>::VoterIsNotRegistered
			);
		});
	}

	#[test]
	fn vote_already_casted() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			Balances::make_free_balance_be(&1, 25u32.into());
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));

			assert_ok!(Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(1)));
			assert_noop!(
				Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(1)),
				Error::<Test>::VoteAlreadyCasted
			);
		});
	}

	#[test]
	fn vote_over_limit() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			Balances::make_free_balance_be(&1, 25u32.into());
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));

			let vote_limit: u32 = VoteLimit::get();
			assert_noop!(
				Voting::vote(
					RuntimeOrigin::signed(1),
					proposal_id,
					VoteDecision::Aye(vote_limit + 1)
				),
				Error::<Test>::VoteAmountLimit
			);
		});
	}

	#[test]
	fn invalid_proposal() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			Balances::make_free_balance_be(&1, 25u32.into());
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));

			assert_noop!(
				Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(2)),
				Error::<Test>::ProposalNotFound
			);
		});
	}

	#[test]
	fn ended_proposal() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			Balances::make_free_balance_be(&1, 25u32.into());
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 10));

			System::set_block_number(20);

			assert_noop!(
				Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(2)),
				Error::<Test>::ProposalAlreadyEnded
			);
		});
	}

	#[test]
	fn invalid_vote_amount() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 90));

			assert_noop!(
				Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(0)),
				Error::<Test>::InvalidVoteAmount
			);
		});
	}
}

mod finish_proposal {
	use super::*;

	#[test]
	fn proposal_passed() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let proposal_id = Voting::get_proposal_counter() + 1;
			Balances::make_free_balance_be(&1, 25u32.into());
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 5));

			assert_ok!(Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(1)));

			System::set_block_number(6);

			assert_ok!(Voting::finish_proposal(RuntimeOrigin::signed(1), proposal_id));
			System::assert_has_event(
				Event::ProposalEnded { proposal_id, status: ProposalStatus::Passed }.into(),
			);
		});
	}

	#[test]
	fn proposal_rejected() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let proposal_id = Voting::get_proposal_counter() + 1;
			Balances::make_free_balance_be(&1, 25u32.into());
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 5));

			assert_ok!(Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Nay(1)));

			System::set_block_number(6);

			assert_ok!(Voting::finish_proposal(RuntimeOrigin::signed(1), proposal_id));
			System::assert_has_event(
				Event::ProposalEnded { proposal_id, status: ProposalStatus::Rejected }.into(),
			);
		});
	}

	#[test]
	fn proposal_tied() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);

			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 5));

			System::set_block_number(6);

			assert_ok!(Voting::finish_proposal(RuntimeOrigin::signed(1), proposal_id));
			System::assert_has_event(
				Event::ProposalEnded { proposal_id, status: ProposalStatus::Tied }.into(),
			);
		});
	}

	#[test]
	fn finish_proposal_fails_if_canceled() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 5));
			assert_ok!(Voting::cancel_proposal(RuntimeOrigin::signed(1), proposal_id));

			assert_noop!(
				Voting::finish_proposal(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::ProposalAlreadyEnded
			);
		});
	}

	#[test]
	fn finish_proposal_early_rejects() {
		new_test_ext().execute_with(|| {
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 5));

			assert_noop!(
				Voting::finish_proposal(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::ProposalAlreadyEnded
			);
		});
	}
}

mod unlock_balance {
	use super::*;

	fn before_each() -> (u32, u32) {
		// initial setup
		System::set_block_number(1);
		let proposal_id = Voting::get_proposal_counter() + 1;
		let initial_balance: u32 = 25;
		Balances::make_free_balance_be(&1, initial_balance.into());
		assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
		assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 5));

		(initial_balance, proposal_id)
	}

	#[test]
	fn unlock_balance() {
		new_test_ext().execute_with(|| {
			let (initial_balance, proposal_id) = before_each();

			assert_ok!(Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(3)));
			System::set_block_number(6);
			assert_ok!(Voting::finish_proposal(RuntimeOrigin::signed(1), proposal_id));

			//try to unlock balance
			assert_ok!(Voting::unlock_balance(RuntimeOrigin::signed(1), proposal_id));
			System::assert_has_event(Event::BalanceUnlocked { proposal_id, who: 1 }.into());
			//Check that the reserved amount from the user is (amount of votes^2)
			let current_balance = Balances::free_balance(&1);
			assert_eq!(initial_balance as u128, current_balance);
		});
	}

	#[test]
	fn cant_unlock_before_proposal_end() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each();

			assert_ok!(Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(3)));

			//try to unlock balance
			assert_noop!(
				Voting::unlock_balance(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::ProposalInProgress
			);
		});
	}
	#[test]
	fn vote_not_found() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each();
			System::set_block_number(6);
			assert_ok!(Voting::finish_proposal(RuntimeOrigin::signed(1), proposal_id));

			//try to unlock balance
			assert_noop!(
				Voting::unlock_balance(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::VoteNotFound
			);
		});
	}

	#[test]
	fn balance_already_unlocked() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each();

			assert_ok!(Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(3)));
			System::set_block_number(6);
			assert_ok!(Voting::finish_proposal(RuntimeOrigin::signed(1), proposal_id));
			//Unlock balance
			assert_ok!(Voting::unlock_balance(RuntimeOrigin::signed(1), proposal_id));

			//Try to unlock again
			assert_noop!(
				Voting::unlock_balance(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::BalanceAlreadyUnocked
			);
		});
	}
}

mod cancel_vote {
	use super::*;

	//Returns (initial_balance, proposal_id)
	fn before_each(time_limit: u64) -> (u32, u32) {
		//Initial setup
		System::set_block_number(1);
		let initial_balance: u32 = 25;
		Balances::make_free_balance_be(&1, initial_balance.into());
		let proposal_id = Voting::get_proposal_counter() + 1;
		assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
		assert_ok!(Voting::make_proposal(
			RuntimeOrigin::signed(1),
			sp_core::H256::zero(),
			time_limit
		));

		assert_ok!(Voting::vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(3)));

		(initial_balance, proposal_id)
	}

	#[test]
	fn cancel_vote_succesfully() {
		new_test_ext().execute_with(|| {
			let (initial_balance, proposal_id) = before_each(40);

			assert_ok!(Voting::cancel_vote(RuntimeOrigin::signed(1), proposal_id));
			System::assert_has_event(Event::VoteCanceled { proposal_id, who: 1 }.into());

			//Check that the reserved amount from the user is (amount of votes^2)
			let current_balance = Balances::free_balance(&1);
			assert_eq!(initial_balance as u128, current_balance);
		});
	}

	#[test]
	fn cant_cancel_after_thresshold() {
		new_test_ext().execute_with(|| {
			//Initial setup
			let threshold = VoteRemovalThreshold::get();
			let (_, proposal_id) = before_each(threshold.into());

			assert_noop!(
				Voting::cancel_vote(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::PassedRemovalThreshold
			);
		});
	}

	#[test]
	fn proposal_not_found() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			Balances::make_free_balance_be(&1, 25u32.into());
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));

			assert_noop!(
				Voting::cancel_vote(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::ProposalNotFound
			);
		});
	}

	#[test]
	fn vote_not_found() {
		new_test_ext().execute_with(|| {
			//Initial setup
			System::set_block_number(1);
			Balances::make_free_balance_be(&1, 25u32.into());
			let proposal_id = Voting::get_proposal_counter() + 1;
			assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
			assert_ok!(Voting::make_proposal(RuntimeOrigin::signed(1), sp_core::H256::zero(), 5));

			assert_noop!(
				Voting::cancel_vote(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::VoteNotFound
			);
		});
	}

	#[test]
	fn ended_proposal() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each(40);

			System::set_block_number(50);

			assert_noop!(
				Voting::cancel_vote(RuntimeOrigin::signed(1), proposal_id),
				Error::<Test>::ProposalAlreadyEnded
			);
		});
	}
}

mod update_vote {
	use super::*;

	fn before_each(proposal_end: u32) -> (u32, u32) {
		System::set_block_number(1);
		let initial_balance: u32 = 25;
		Balances::make_free_balance_be(&1, initial_balance.into());
		let proposal_id = Voting::get_proposal_counter() + 1;
		assert_ok!(Voting::register_voter(RuntimeOrigin::root(), 1));
		assert_ok!(Voting::make_proposal(
			RuntimeOrigin::signed(1),
			sp_core::H256::zero(),
			proposal_end.into()
		));

		(initial_balance, proposal_id)
	}

	#[test]
	fn increase_yes_vote() {
		new_test_ext().execute_with(|| {
			let (initial_balance, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));

			let proposal_before_update = Voting::get_proposal(&proposal_id).unwrap();

			assert_ok!(Voting::update_vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount + 1)
			));
			System::assert_has_event(
				Event::<Test>::VoteUpdated {
					proposal_id,
					who: 1,
					previous: VoteDecision::Aye(vote_amount),
					new: VoteDecision::Aye(vote_amount + 1),
				}
				.into(),
			);

			let proposal_after_update = Voting::get_proposal(&proposal_id).unwrap();

			//Check that the reserved amount from the user is (amount of votes^2)
			let current_balance = Balances::free_balance(&1);
			assert_eq!(
				initial_balance as u128,
				current_balance + ((vote_amount + 1) as u128).pow(2)
			);
			assert_eq!(proposal_before_update.ayes + 1, proposal_after_update.ayes);
		});
	}

	#[test]
	fn decrease_yes() {
		new_test_ext().execute_with(|| {
			let (initial_balance, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));

			let proposal_before_update = Voting::get_proposal(&proposal_id).unwrap();

			assert_ok!(Voting::update_vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount - 1)
			));
			System::assert_has_event(
				Event::<Test>::VoteUpdated {
					proposal_id,
					who: 1,
					previous: VoteDecision::Aye(vote_amount),
					new: VoteDecision::Aye(vote_amount - 1),
				}
				.into(),
			);

			let proposal_after_update = Voting::get_proposal(&proposal_id).unwrap();

			//Check that the reserved amount from the user is (amount of votes^2)
			let current_balance = Balances::free_balance(&1);
			assert_eq!(
				initial_balance as u128,
				current_balance + ((vote_amount - 1) as u128).pow(2)
			);
			assert_eq!(proposal_before_update.ayes - 1, proposal_after_update.ayes);
		});
	}

	#[test]
	fn increse_no_vote() {
		new_test_ext().execute_with(|| {
			let (initial_balance, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Nay(vote_amount)
			));

			let proposal_before_update = Voting::get_proposal(&proposal_id).unwrap();

			assert_ok!(Voting::update_vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Nay(vote_amount + 1)
			));
			System::assert_has_event(
				Event::<Test>::VoteUpdated {
					proposal_id,
					who: 1,
					previous: VoteDecision::Nay(vote_amount),
					new: VoteDecision::Nay(vote_amount + 1),
				}
				.into(),
			);

			let proposal_after_update = Voting::get_proposal(&proposal_id).unwrap();

			//Check that the reserved amount from the user is (amount of votes^2)
			let current_balance = Balances::free_balance(&1);
			assert_eq!(
				initial_balance as u128,
				current_balance + ((vote_amount + 1) as u128).pow(2)
			);
			assert_eq!(proposal_before_update.nays + 1, proposal_after_update.nays);
		});
	}

	#[test]
	fn decrease_no_vote() {
		new_test_ext().execute_with(|| {
			let (initial_balance, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Nay(vote_amount)
			));

			let proposal_before_update = Voting::get_proposal(&proposal_id).unwrap();

			assert_ok!(Voting::update_vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Nay(vote_amount - 1)
			));
			System::assert_has_event(
				Event::<Test>::VoteUpdated {
					proposal_id,
					who: 1,
					previous: VoteDecision::Nay(vote_amount),
					new: VoteDecision::Nay(vote_amount - 1),
				}
				.into(),
			);

			let proposal_after_update = Voting::get_proposal(&proposal_id).unwrap();

			//Check that the reserved amount from the user is (amount of votes^2)
			let current_balance = Balances::free_balance(&1);
			assert_eq!(
				initial_balance as u128,
				current_balance + ((vote_amount - 1) as u128).pow(2)
			);
			assert_eq!(proposal_before_update.nays - 1, proposal_after_update.nays);
		});
	}

	#[test]
	fn change_from_yes_to_no() {
		new_test_ext().execute_with(|| {
			let (initial_balance, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));

			let proposal_before_update = Voting::get_proposal(&proposal_id).unwrap();

			assert_ok!(Voting::update_vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Nay(vote_amount)
			));
			System::assert_has_event(
				Event::<Test>::VoteUpdated {
					proposal_id,
					who: 1,
					previous: VoteDecision::Aye(vote_amount),
					new: VoteDecision::Nay(vote_amount),
				}
				.into(),
			);

			let proposal_after_update = Voting::get_proposal(&proposal_id).unwrap();

			//Check that the reserved amount from the user is (amount of votes^2)
			let current_balance = Balances::free_balance(&1);
			assert_eq!(initial_balance as u128, current_balance + ((vote_amount) as u128).pow(2));
			assert_eq!(proposal_before_update.ayes, proposal_after_update.ayes + vote_amount);
			assert_eq!(proposal_before_update.nays, proposal_after_update.nays - vote_amount);
		});
	}

	#[test]
	fn vote_over_limit() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));

			let vote_limit: u32 = VoteLimit::get();
			assert_noop!(
				Voting::update_vote(
					RuntimeOrigin::signed(1),
					proposal_id,
					VoteDecision::Aye(vote_limit + 1)
				),
				Error::<Test>::VoteAmountLimit
			);
		});
	}
	#[test]
	fn vote_not_found() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;

			assert_noop!(
				Voting::update_vote(
					RuntimeOrigin::signed(1),
					proposal_id,
					VoteDecision::Aye(vote_amount)
				),
				Error::<Test>::VoteNotFound
			);
		});
	}

	#[test]
	fn proposal_ended() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));

			System::set_block_number(51);
			assert_noop!(
				Voting::update_vote(
					RuntimeOrigin::signed(1),
					proposal_id,
					VoteDecision::Aye(vote_amount)
				),
				Error::<Test>::ProposalAlreadyEnded
			);
		});
	}

	#[test]
	fn reduction_fails_passed_threshold() {
		new_test_ext().execute_with(|| {
			let threshold = VoteRemovalThreshold::get();
			let (_, proposal_id) = before_each(threshold - 1);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));

			assert_noop!(
				Voting::update_vote(
					RuntimeOrigin::signed(1),
					proposal_id,
					VoteDecision::Aye(vote_amount - 1)
				),
				Error::<Test>::PassedRemovalThreshold
			);
		});
	}

	#[test]
	fn increase_works_passed_threshold() {
		new_test_ext().execute_with(|| {
			let threshold = VoteRemovalThreshold::get();
			let (_, proposal_id) = before_each(threshold - 1);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));

			assert_ok!(Voting::update_vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount + 1)
			),);
		});
	}

	#[test]
	fn invalid_update_amount() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));

			assert_noop!(
				Voting::update_vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(0)),
				Error::<Test>::InvalidUpdateAmount
			);
		});
	}

	#[test]
	fn not_enough_balance() {
		new_test_ext().execute_with(|| {
			let (_, proposal_id) = before_each(50);

			let vote_amount: u32 = 3;
			assert_ok!(Voting::vote(
				RuntimeOrigin::signed(1),
				proposal_id,
				VoteDecision::Aye(vote_amount)
			));
			assert_noop!(
				Voting::update_vote(RuntimeOrigin::signed(1), proposal_id, VoteDecision::Aye(6)),
				pallet_balances::Error::<Test>::InsufficientBalance
			);
		});
	}
}
