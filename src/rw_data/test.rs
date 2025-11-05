use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

type TrainId = String;
type StationId = String;
type OperationNumber = String;

/// 统一后的一个运用链：同一个运番下连续的列车片段
#[derive(Debug, Default)]
pub struct OperationChain {
    pub number: OperationNumber,
    pub segments: BTreeSet<OperationSegment>,
}

/// 运用表中的“片段”，对应一列车的一段运行
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OperationSegment {
    pub train_id: TrainId,
    pub start_station: StationId,
    pub end_station: StationId,
    pub start_time: i64, // 秒（或自定义 Time 类型）
    pub end_time: i64,
}

impl Ord for OperationSegment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.start_time
            .cmp(&other.start_time)
            .then_with(|| self.train_id.cmp(&other.train_id))
    }
}
impl PartialOrd for OperationSegment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// 列车的描述（从既有数据结构抽象而来）
#[derive(Debug, Clone)]
pub struct Train {
    pub id: TrainId,
    pub segment: OperationSegment,
    /// 起始时带来的运番（同一列车可能携带多个运番；空表示由上游决定）
    pub start_numbers: Vec<Vec<OperationNumber>>,
    pub links: Vec<Link>,
}

/// 列车之间的衔接关系
#[derive(Debug, Clone)]
pub enum Link {
    /// 常规接续（同运番继续跑下一列车）
    Continue { to: TrainId },
    /// 増结：把 `to` 列车并入当前运番
    Connect {
        to: TrainId,
        sub_numbers: Vec<OperationNumber>,
        connect_to_front: bool,
    },
    /// 解结：分离出的列车独立成新运番
    Release {
        detached_to: TrainId,
        detached_numbers: Vec<OperationNumber>,
    },
    /// 改番：同列车后续运番变化
    NumberChange {
        to: TrainId,
        new_numbers: Vec<OperationNumber>,
        reverse: bool,
    },
    /// 终点（入区/路外终到等），链在此结束
    End,
}

#[derive(Debug)]
pub enum AssembleError {
    TrainNotFound(TrainId),
    NoOperationNumbers(TrainId),
}

impl fmt::Display for AssembleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssembleError::TrainNotFound(id) => write!(f, "train `{}` not found", id),
            AssembleError::NoOperationNumbers(id) => {
                write!(f, "train `{}` has no operation number to continue", id)
            }
        }
    }
}

impl std::error::Error for AssembleError {}

/// 负责把所有列车串成运用
pub struct OperationAssembler<'a> {
    trains: BTreeMap<&'a TrainId, &'a Train>,
    chains: BTreeMap<OperationNumber, OperationChain>,
    visited: BTreeSet<(TrainId, OperationNumber)>,
}

impl<'a> OperationAssembler<'a> {
    pub fn new(trains: &'a [Train]) -> Self {
        let map = trains.iter().map(|t| (&t.id, t)).collect();
        Self {
            trains: map,
            chains: BTreeMap::new(),
            visited: BTreeSet::new(),
        }
    }

    /// 主入口：返回“运番 -> 运用链”
    pub fn assemble(mut self) -> Result<BTreeMap<OperationNumber, OperationChain>, AssembleError> {
        let mut queue: VecDeque<TraversalState> = VecDeque::new();

        // Step 1：把所有带起点运番的列车入队
        for train in self.trains.values() {
            for numbers in &train.start_numbers {
                queue.push_back(TraversalState {
                    train_id: train.id.clone(),
                    numbers: numbers.clone(),
                });
            }
        }

        // Step 2：BFS/DFS 方式扫所有列车
        while let Some(state) = queue.pop_front() {
            self.spawn_chain(&state.train_id, state.numbers.clone())?;

            let train = self
                .trains
                .get(&state.train_id.as_str())
                .ok_or_else(|| AssembleError::TrainNotFound(state.train_id.clone()))?;

            for link in &train.links {
                match link {
                    Link::Continue { to } => {
                        queue.push_back(TraversalState {
                            train_id: to.clone(),
                            numbers: state.numbers.clone(),
                        });
                    }
                    Link::Connect {
                        to,
                        sub_numbers,
                        connect_to_front,
                    } => {
                        let mut merged = if *connect_to_front {
                            let mut v = sub_numbers.clone();
                            v.extend(state.numbers.clone());
                            v
                        } else {
                            let mut v = state.numbers.clone();
                            v.extend(sub_numbers.clone());
                            v
                        };
                        // 去重，保持原顺
                        merged = dedup_preserve_order(merged);
                        queue.push_back(TraversalState {
                            train_id: to.clone(),
                            numbers: merged,
                        });
                    }
                    Link::Release {
                        detached_to,
                        detached_numbers,
                    } => {
                        // 主编成继续现有运番
                        queue.push_back(TraversalState {
                            train_id: detached_to.clone(),
                            numbers: dedup_preserve_order(detached_numbers.clone()),
                        });
                    }
                    Link::NumberChange {
                        to,
                        new_numbers,
                        reverse,
                    } => {
                        let mut updated = new_numbers.clone();
                        if *reverse {
                            updated.reverse();
                        }
                        queue.push_back(TraversalState {
                            train_id: to.clone(),
                            numbers: updated,
                        });
                    }
                    Link::End => {
                        // nothing — 运用在此终止
                    }
                }
            }
        }

        Ok(self.chains)
    }

    /// 把当前列车片段加入所有对应的运番链
    fn spawn_chain(
        &mut self,
        train_id: &TrainId,
        numbers: Vec<OperationNumber>,
    ) -> Result<(), AssembleError> {
        if numbers.is_empty() {
            return Err(AssembleError::NoOperationNumbers(train_id.clone()));
        }

        let train = self
            .trains
            .get(&train_id.as_str())
            .ok_or_else(|| AssembleError::TrainNotFound(train_id.clone()))?;

        for number in numbers {
            if !self.visited.insert((train_id.clone(), number.clone())) {
                // 同一列车 + 运番 已处理过，避免循环
                continue;
            }
            let chain = self
                .chains
                .entry(number.clone())
                .or_insert_with(|| OperationChain {
                    number: number.clone(),
                    ..Default::default()
                });
            chain.segments.insert(train.segment.clone());
        }
        Ok(())
    }
}

/// 队列中携带的状态
#[derive(Debug, Clone)]
struct TraversalState {
    train_id: TrainId,
    numbers: Vec<OperationNumber>,
}

/// 去重但保持原有顺序
fn dedup_preserve_order(mut numbers: Vec<OperationNumber>) -> Vec<OperationNumber> {
    let mut seen = BTreeSet::new();
    numbers.retain(|n| seen.insert(n.clone()));
    numbers
}
