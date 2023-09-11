use anyhow::Result;
use aptos_events::UserQuery;
use halo2_proofs::halo2curves::pasta::{EqAffine, Fp};
use halo2_proofs::poly::commitment::ParamsProver;
use halo2_proofs::poly::ipa::commitment::ParamsIPA;
use zkmove_vm_circuit::circuit::VmCircuit;
use zkmove_vm_circuit::witness::Witness;
use zkmove_vm_circuit::{find_best_k, prove_vm_circuit_ipa, setup_vm_circuit};

pub fn prove(_query: UserQuery, witness: Witness<Fp>) -> Result<Vec<u8>> {
    let circuit = VmCircuit { witness };
    let k = find_best_k::<Fp, _>(&circuit, vec![])?;
    let params: ParamsIPA<EqAffine> = ParamsIPA::new(k);
    let (_vk, pk) = setup_vm_circuit(&circuit, &params)?;
    // TODO: check vk is equal to vk stored onchain.
    let proof = prove_vm_circuit_ipa(circuit, &[], &params, pk)?;
    Ok(proof)
}
