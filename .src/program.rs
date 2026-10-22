//! The compiled form of a Work Process: what a design becomes at design
//! time, whatever format it was drawn in (ADR-0066, amendment 2026-10-22).
//!
//! A [`Program`] is a graph of steps, each a [`Node`] numbered by its place,
//! every flow already resolved to the number of the node it leads to and
//! every embedded subprocess already laid flat into [`Scope`]s. Nothing in
//! it names a design format: a BPMN 2.0 diagram and any later format
//! compile to the same nodes, and the engine ([`crate::flow`]) runs these
//! and nothing else. It is kept in its binary form ([`crate::form`]) inside
//! the Work Process unit a node loads, read once as the unit opens.

use assign::Assignment;
use path::expression::Value;

/// One compiled Work Process.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    /// Its name, as the design gave it.
    pub name: String,
    /// Every step, numbered by its place.
    pub nodes: Vec<Node>,
    /// Every scope a step may stand in: a task with a boundary event of its
    /// own, or an embedded subprocess laid flat.
    pub scopes: Vec<Scope>,
    /// Where a Message starts an Instance.
    pub starts: Vec<u32>,
    /// The correlation sets it follows, by the names the cluster's
    /// `[[correlation_sets]]` declares them by (ADR-0024 and ADR-0066,
    /// amendments 2026-10-22). A set's keys are the cluster's, never the
    /// design's: what makes a Message's key in it is read where the set is
    /// declared, so every Work Process following it agrees.
    pub sets: Vec<String>,
}

/// One step of a [`Program`].
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// The design's identifier for it, so a refusal and an audit name it.
    pub id: String,
    /// The innermost scope it stands in.
    pub scope: Option<u32>,
    /// The correlation set, of [`Program::sets`], a Start or a step waiting
    /// for a Message takes its Message by.
    pub set: Option<u32>,
    pub step: Step,
}

/// What a step does.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Where a Message starts an Instance.
    Start { next: u32 },
    /// Work on the current Message, which makes the next one.
    Call { work: Work, next: u32 },
    /// The current Message published back through routing.
    Publish { next: u32 },
    /// Waiting for the next Message of the Instance's key.
    Wait { next: u32 },
    /// Messages of the key collected, until `most` have come or `within`
    /// is due, then made one Message whose Sections are theirs.
    Batch {
        most: u32,
        within: Option<Due>,
        next: u32,
    },
    /// Waiting for a time.
    Timer { due: Due, next: u32 },
    /// The first branch whose condition holds, else `otherwise`.
    Decide {
        branches: Vec<Branch>,
        otherwise: Option<u32>,
    },
    /// Every flow taken at once.
    Fork { next: Vec<u32> },
    /// Waiting until `from` flows have arrived, then one goes on.
    Join { from: u32, next: u32 },
    /// The end of a flow.
    End { ending: Ending },
}

/// The work a [`Step::Call`] does.
#[derive(Clone, Debug, PartialEq)]
pub enum Work {
    /// A call to a compiled unit, by module and unit: a rule, a
    /// Transformation, anything code decides.
    Unit(Call),
    /// Context values set: a new Message generation (`doc/terminology.md`,
    /// *Assignment*).
    Assign(Vec<Assignment>),
    /// The current Message split into parts by the splitting unit named,
    /// each part published back through routing.
    Debatch(Call),
}

/// A call to a compiled unit: the `[[modules]]` entry holding it, the unit
/// as that module's manifest names it, and what the call gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub module: String,
    pub unit: String,
    pub settings: Vec<(String, Value)>,
}

/// One branch of a [`Step::Decide`]: its condition, in Xmip's expression
/// language, and where it leads.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    pub condition: String,
    pub next: u32,
}

/// When a timer is due.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    /// This long after the timer starts.
    After { nanos: u64 },
    /// At this time.
    At { unix_nanos: i128 },
}

/// How a flow ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ending {
    /// This flow ends; the Instance completes once none is left.
    Flow,
    /// The Instance ends now, every flow with it.
    Terminate,
    /// An error is thrown, caught by the innermost scope catching its code.
    Error { code: String },
}

/// A scope: what interrupts the steps standing in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    pub id: String,
    pub parent: Option<u32>,
    /// The errors it catches, in the order the design declared them.
    pub catches: Vec<Catch>,
    /// The timer that interrupts it, started as a flow enters it.
    pub timeout: Option<Timeout>,
}

/// An error caught: `code`, or any where it names none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catch {
    pub code: Option<String>,
    pub next: u32,
}

/// A scope's timer and where it leads once due.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeout {
    pub due: Due,
    pub next: u32,
}

impl Step {
    /// Every node this step may lead to.
    #[must_use]
    pub fn leads(&self) -> Vec<u32> {
        match self {
            Self::Start { next }
            | Self::Call { next, .. }
            | Self::Publish { next }
            | Self::Wait { next }
            | Self::Batch { next, .. }
            | Self::Timer { next, .. }
            | Self::Join { next, .. } => vec![*next],
            Self::Decide {
                branches,
                otherwise,
            } => branches
                .iter()
                .map(|branch| branch.next)
                .chain(*otherwise)
                .collect(),
            Self::Fork { next } => next.clone(),
            Self::End { .. } => Vec::new(),
        }
    }
}

impl Program {
    /// Whether the program holds together: every number names a node or a
    /// scope there is, a scope's parent comes before it, a Join waits for
    /// at least one flow and a Batch for at least one Message, and every
    /// condition compiles. Checked as a design compiles and again as a unit
    /// opens, so a unit built by hand is held to the same.
    ///
    /// # Errors
    /// Every problem, each naming the node or scope.
    pub fn check(&self) -> Result<(), Vec<String>> {
        let mut problems = Vec::new();
        let nodes = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);
        let scopes = u32::try_from(self.scopes.len()).unwrap_or(u32::MAX);
        if self.starts.is_empty() {
            problems.push(format!("the Work Process '{}' has no start", self.name));
        }
        for start in &self.starts {
            if *start >= nodes {
                problems.push(format!("a start names node {start}, which is not there"));
            }
        }
        for (place, node) in self.nodes.iter().enumerate() {
            let named = |what: &str| format!("'{}' {what}", node.id);
            if node.scope.is_some_and(|scope| scope >= scopes) {
                problems.push(named("stands in a scope that is not there"));
            }
            if node.set.is_some_and(|set| set as usize >= self.sets.len()) {
                problems.push(named("follows a correlation set that is not there"));
            }
            for next in node.step.leads() {
                if next >= nodes {
                    problems.push(named(&format!("leads to node {next}, which is not there")));
                }
                if usize::try_from(next).is_ok_and(|next| next == place) {
                    problems.push(named("leads to itself"));
                }
            }
            match &node.step {
                Step::Join { from: 0, .. } => problems.push(named("joins no flow")),
                Step::Batch { most: 0, .. } => problems.push(named("batches no Message")),
                Step::Decide { branches, .. } => {
                    for branch in branches {
                        if let Err(why) = path::expression::Expression::parse(&branch.condition) {
                            problems.push(named(&format!("has a condition that {why}")));
                        }
                    }
                }
                _ => {}
            }
        }
        for (place, scope) in self.scopes.iter().enumerate() {
            let before = u32::try_from(place).unwrap_or(u32::MAX);
            if scope.parent.is_some_and(|parent| parent >= before) {
                problems.push(format!("the scope '{}' names a parent after it", scope.id));
            }
            let leads = scope.catches.iter().map(|catch| catch.next);
            for next in leads.chain(scope.timeout.map(|timeout| timeout.next)) {
                if next >= nodes {
                    problems.push(format!("the scope '{}' leads to node {next}", scope.id));
                }
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    /// The scopes `node` stands in, innermost first.
    #[must_use]
    pub fn scopes_of(&self, node: u32) -> Vec<u32> {
        let mut scopes = Vec::new();
        let mut at = self.node(node).and_then(|node| node.scope);
        while let Some(scope) = at {
            scopes.push(scope);
            at = self
                .scopes
                .get(usize::try_from(scope).unwrap_or(usize::MAX))
                .and_then(|scope| scope.parent);
        }
        scopes
    }

    /// The node at `place`.
    #[must_use]
    pub fn node(&self, place: u32) -> Option<&Node> {
        self.nodes.get(usize::try_from(place).ok()?)
    }

    /// The scope at `place`.
    #[must_use]
    pub fn scope(&self, place: u32) -> Option<&Scope> {
        self.scopes.get(usize::try_from(place).ok()?)
    }

    /// Every name its conditions read, each once, in the order first read:
    /// what the runtime reads of a Message before it hands it on.
    #[must_use]
    pub fn reads(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for node in &self.nodes {
            if let Step::Decide { branches, .. } = &node.step {
                for branch in branches {
                    let Ok(expression) = path::expression::Expression::parse(&branch.condition)
                    else {
                        continue;
                    };
                    for name in expression.names() {
                        if !names.iter().any(|known| known == name) {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
        names
    }
}
