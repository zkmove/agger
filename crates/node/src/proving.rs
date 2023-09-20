use anyhow::{ensure, Result};
use halo2_proofs::halo2curves::bn256::{Bn256, Fr};
use halo2_proofs::poly::kzg::commitment::ParamsKZG;
use halo2_proofs::SerdeFormat;
use zkmove_vm_circuit::circuit::VmCircuit;
use zkmove_vm_circuit::witness::Witness;
use zkmove_vm_circuit::{prove_vm_circuit_kzg, setup_vm_circuit};

use agger_contract_types::UserQuery;

use crate::vk_generator::VerificationParameters;

pub fn prove(
    _query: UserQuery,
    witness: Witness<Fr>,
    verification_parameters: &VerificationParameters,
) -> Result<Vec<u8>> {
    let circuit = VmCircuit { witness };
    let params = ParamsKZG::<Bn256>::read_custom(
        &mut verification_parameters.param.as_slice(),
        SerdeFormat::Processed,
    )?;
    let (vk, pk) = setup_vm_circuit(&circuit, &params)?;

    // check vk is equal to vk stored onchain.
    ensure!(
        vk.to_bytes(SerdeFormat::Processed) == verification_parameters.vk,
        "vk equality checking failure"
    );
    let proof = prove_vm_circuit_kzg(circuit, &[], &params, pk)?;
    Ok(proof)
}
