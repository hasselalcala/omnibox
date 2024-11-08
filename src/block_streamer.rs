use near_sdk::AccountId;
use std::str::FromStr;
use std::sync::Arc;

use near_jsonrpc_client::errors::{JsonRpcError, JsonRpcServerError};
use near_jsonrpc_client::methods::{block::RpcBlockError, chunk::ChunkReference};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::transactions::RpcTransactionResponse;
use near_primitives::hash::CryptoHash;
use near_primitives::types::{BlockId, BlockReference, Finality};
use near_primitives::views::{ActionView, BlockView, ChunkView, FinalExecutionOutcomeViewEnum};

use crate::constants::*;
use crate::event::*;
use crate::TransactionProcessor;

pub fn specify_block_reference(last_processed_block: u64) -> BlockReference {
    if last_processed_block == 0 {
        BlockReference::Finality(Finality::Final)
    } else {
        BlockReference::BlockId(BlockId::Height(last_processed_block + 1))
    }
}

pub async fn fetch_block(
    client: &JsonRpcClient,
    block_reference: BlockReference,
) -> Result<BlockView, JsonRpcError<RpcBlockError>> {
    let block_request = methods::block::RpcBlockRequest { block_reference };
    let block_response = client.call(block_request).await;
    block_response
}

pub async fn fetch_chunk(
    client: &JsonRpcClient,
    chunk_hash: CryptoHash,
) -> Result<ChunkView, Box<dyn std::error::Error + Send + Sync>> {
    let chunk_reference = ChunkReference::ChunkHash {
        chunk_id: chunk_hash,
    };

    let chunk_request = methods::chunk::RpcChunkRequest {
        chunk_reference: chunk_reference,
    };

    let chunk_response = client.call(chunk_request).await;

    match chunk_response {
        Ok(chunk_details) => Ok(chunk_details),
        Err(err) => match err.handler_error() {
            Some(methods::chunk::RpcChunkError::UnknownChunk { .. }) => {
                println!("(i) Unknown chunk!");
                panic!("Unknown chunk!");
            }
            Some(err) => {
                println!("(i) An error occurred `{:#?}`", err);
                panic!("Other error!");
            }
            _ => {
                println!("(i) A non-handler error occurred `{:#?}`", err);
                panic!("Non handled error!");
            }
        },
    }
}

pub async fn find_transaction_in_block(
    client: &JsonRpcClient,
    block: &BlockView,
    account_id: &str,
    method_name: &str,
) -> Result<Option<(String, AccountId)>, Box<dyn std::error::Error + Send + Sync>> {
    for chunk_header in &block.chunks {
        let chunk_hash = chunk_header.chunk_hash;
        let chunk = fetch_chunk(client, chunk_hash).await?;

        for transaction in &chunk.transactions {
            if transaction.receiver_id == account_id {
                for action in &transaction.actions {
                    if let ActionView::FunctionCall {
                        method_name: action_method_name,
                        ..
                    } = action
                    {
                        if action_method_name == method_name {
                            return Ok(Some((
                                transaction.hash.to_string(),
                                transaction.signer_id.clone(),
                            )));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

pub async fn get_logs(
    client: &JsonRpcClient,
    tx_hash: &str,
    sender_account_id: &AccountId,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let tx_hash =
        CryptoHash::from_str(tx_hash).map_err(|e| format!("Failed to parse tx_hash: {}", e))?;


    let transaction_status_request = methods::tx::RpcTransactionStatusRequest {
        transaction_info: methods::tx::TransactionInfo::TransactionId {
            tx_hash,
            sender_account_id: sender_account_id.clone(),
        },
        wait_until: near_primitives::views::TxExecutionStatus::None,
    };

    let transaction_status_response = client.call(transaction_status_request).await?;
    println!("\nExtracting logs...");
    let logs = extract_logs(&transaction_status_response);
    println!("Logs: {:?}", logs);
    Ok(logs)
}

pub fn extract_logs(response: &RpcTransactionResponse) -> Vec<String> {
    let mut logs = Vec::new();

    if let Some(final_outcome_enum) = &response.final_execution_outcome {
        match final_outcome_enum {
            FinalExecutionOutcomeViewEnum::FinalExecutionOutcome(final_outcome) => {
                logs.extend(final_outcome.transaction_outcome.outcome.logs.clone());

                for receipt_outcome in &final_outcome.receipts_outcome {
                    logs.extend(receipt_outcome.outcome.logs.clone());
                }
            }
            FinalExecutionOutcomeViewEnum::FinalExecutionOutcomeWithReceipt(
                final_outcome_with_receipt,
            ) => {
                // How we manage this case?
                println!("Something is missing: {:?}", final_outcome_with_receipt);
            }
        }
    }

    logs
}

fn process_log(log: &str) -> Result<EventData, Box<dyn std::error::Error + Send + Sync>> {
    //println!("Raw log: {}", log);
    let json_start = log
        .find("{")
        .ok_or_else(|| String::from("JSON not found in log"))?;
    let json_str = &log[json_start..];
    //println!("Extracted JSON string: {}", json_str);

    let event_log: EventLog = match serde_json::from_str(json_str) {
        Ok(log) => log,
        Err(e) => {
            println!("Error deserializing JSON: {}", e);
            return Err(Box::new(e));
        }
    };

    let event_data = event_log
        .data
        .into_iter()
        .next()
        .ok_or_else(|| String::from("No EventData found in log"))?;

    //println!("Deserialized yield_id: {:?}", event_data.yield_id);

    Ok(event_data)
}

pub async fn start_polling(
    client: &JsonRpcClient,
    mut last_processed_block: u64,
    processor: Arc<dyn TransactionProcessor>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting polling...");

    loop {
        println!("Last processed block: {}", last_processed_block);

        let block_reference = specify_block_reference(last_processed_block);
        //println!("Block reference: {:?}", block_reference);
        match fetch_block(client, block_reference).await {
            Ok(block) => {
                println!("Processing block: {:#?}", block.header.height);

                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if let Some((tx_hash, sender_account_id)) = find_transaction_in_block(
                    &client,
                    &block,
                    ACCOUNT_TO_LISTEN,
                    FUNCTION_TO_LISTEN,
                )
                .await?
                {
                    //println!("Transaction found: {:?}", tx_hash);
                    let logs = get_logs(client, &tx_hash, &sender_account_id).await?;
                    //println!("Logs: {:?}", logs);
                    if logs.len() > 2 {
                        panic!("More than one log found. Contract might have changed!");
                    }

                    if let Some(log) = logs.first() {
                        if let Ok(event) = process_log(log) {
                            let processor_clone = Arc::clone(&processor);

                            println!("\nEmitted event: {:?}\n", event);

                            tokio::spawn(async move {
                                if let Err(e) = processor_clone.process_transaction(event).await {
                                    eprintln!("Error processing transaction: {}", e);
                                }
                            });
                        } else {
                            println!("Failed to process log");
                        }
                    } else {
                        println!("No logs found for this transaction");
                    }

                }

                // Actualizar el last_processed_block al nuevo bloque procesado
                last_processed_block = block.header.height;
                println!("Saved new block height: {}", last_processed_block);
            }
            Err(err) => match err.handler_error() {
                Some(methods::block::RpcBlockError::UnknownBlock { .. }) => {
                    println!("(i) Unknown block!");
                    last_processed_block += 1; // Incrementar el bloque procesado
                    println!("Saved new block height: {}", last_processed_block);
                }
                Some(err) => {
                    println!("(i) An error occurred `{:#?}`", err);
                    panic!("Other error!");
                }
                _ => match err {
                    JsonRpcError::ServerError(JsonRpcServerError::ResponseStatusError(status)) => {
                        println!("(i) Server error occurred: status code {}", status);

                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                    _ => {
                        println!("(i) A non-handler error occurred `{:#?}`", err);
                        panic!("Non handled error!");
                    }
                },
            },
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
