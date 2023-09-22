use anyhow::{ensure, Result};
use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use halo2_proofs::halo2curves::bn256::{Bn256, Fr};
use halo2_proofs::poly::kzg::commitment::ParamsKZG;
use halo2_proofs::SerdeFormat;
use log::{error, info};
use threadpool::ThreadPool;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use zkmove_vm_circuit::circuit::VmCircuit;
use zkmove_vm_circuit::witness::Witness;
use zkmove_vm_circuit::{prove_vm_circuit_kzg, setup_vm_circuit};

use agger_contract_types::UserQuery;

use crate::query_runner::witness;
use crate::vk_generator::{CountingRng, VerificationParameters};

#[derive(Clone, Debug)]
pub struct ProveTask {
    pub query: UserQuery,
    pub modules: Vec<Vec<u8>>,
    pub vp: VerificationParameters,
}

#[derive(Debug)]
pub struct ProvingTaskDispatcher {
    task_receiver: Receiver<ProveTask>,
    output_sender: Sender<(UserQuery, Result<Vec<u8>>)>,
    threadpool: ThreadPool,
}

impl ProvingTaskDispatcher {
    pub fn new(
        threadpool: ThreadPool,
        task_receiver: Receiver<ProveTask>,
        output_sender: Sender<(UserQuery, Result<Vec<u8>>)>,
    ) -> Self {
        Self {
            output_sender,
            threadpool,
            task_receiver,
        }
    }
    pub async fn run(mut self) {
        let mut fs = FuturesUnordered::new();
        loop {
            if self.threadpool.active_count() == self.threadpool.max_count() {
                // when ongoing tasks is full, we wait on one task finishing, then go on to receive new tasks.
                if let Some(received_output) = fs.next().await {
                    match received_output {
                        Ok(output) => {
                            if let Err(_) = self.output_sender.send(output).await {
                                break;
                            }
                        }
                        Err(_) => {
                            error!("task ended without sending result");
                        }
                    }
                }
            }
            let task = tokio::select! {
                received_output = fs.next() => {
                    match received_output {
                        Some(Ok(output)) => {
                            if let Err(_) = self.output_sender.send(output).await {
                                // output receiver is gone, stop dispatching
                                break
                            }
                            continue
                        }
                        Some(Err(_)) => {
                            error!("task ended without sending result");
                            continue
                        }
                        None => {
                            info!("empty task queues, wait for new task");
                            self.task_receiver.recv().await
                        }
                    }
                }
                task = self.task_receiver.recv() => {
                    task
                }
            };
            if let Some(task) = task {
                info!(
                    "new query task, user: {:#x}, id: {}",
                    task.query.user, task.query.id
                );
                let (tx, rx) = oneshot::channel();
                fs.push(rx);
                self.threadpool.execute(|| {
                    let query = task.query.clone();
                    let output = run_task(task);
                    if let Err(_) = tx.send((query, output)) {
                        error!("task ended, but output receiver is lost");
                    }
                });
            } else {
                // all task sender is gone, just stop dispatching.
                break;
            }
        }
        info!("prove dispatcher is closing...");
        // before closing, wait any queued or ongoing computations to finish.
        while let Some(received_output) = fs.next().await {
            match received_output {
                Ok(output) => {
                    if let Err(_) = self.output_sender.send(output).await {
                        break;
                    }
                }
                Err(_) => {
                    error!("task ended without sending result");
                }
            }
        }
        info!("prove dispatcher is closed");
    }
}

fn run_task(ProveTask { query, modules, vp }: ProveTask) -> Result<Vec<u8>> {
    let witness = witness(query.clone(), modules, &vp)?;
    prove(query, witness, &vp)
}

pub fn prove(
    _query: UserQuery,
    witness: Witness<Fr>,
    verification_parameters: &VerificationParameters,
) -> Result<Vec<u8>> {
    let circuit = VmCircuit { witness };
    // let params = ParamsKZG::<Bn256>::read_custom(
    //     &mut verification_parameters.param.as_slice(),
    //     SerdeFormat::Processed,
    // )?;
    let params = ParamsKZG::<Bn256>::setup(
        bcs::from_bytes(verification_parameters.param.as_slice())?,
        CountingRng(42),
    );
    let (vk, pk) = setup_vm_circuit(&circuit, &params)?;

    // check vk is equal to vk stored onchain.
    ensure!(
        vk.to_bytes(SerdeFormat::Processed) == verification_parameters.vk,
        "vk equality checking failure"
    );
    let proof = prove_vm_circuit_kzg(circuit, &[], &params, pk)?;
    Ok(proof)
}
