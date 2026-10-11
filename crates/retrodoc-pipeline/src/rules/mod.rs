//! Business rules pass (ADR 0021, `issues/business_rules.md`): what is allowed, refused, limited,
//! and in which states, stated in business language with its evidence.
//!
//! No pattern of any language or framework is written here: the LLM judges what is a rule on the
//! code it is shown. What is mechanical is the choice of that code and of the evidence around it
//! ([`RuleInput`]), the same for every stack.

mod input;

pub use self::input::RuleInput;
