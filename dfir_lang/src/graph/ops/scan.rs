use quote::quote_spanned;

use super::{
    OperatorCategory, OperatorConstraints, OperatorWriteOutput, RANGE_0,
    RANGE_1, WriteContextArgs,
};

/// > 1 input stream, 1 output stream
///
/// > Arguments: two arguments, both closures. The first closure is used to create the initial
/// > value for the accumulator, and the second is used to transform new items with the existing
/// > accumulator value. The second closure takes two arguments: an `&mut Accum` accumulated
/// > value, and an `Item`, and returns an `Option<o>` that will be emitted to the output stream
/// > if it's `Some`, or terminate the stream if it's `None`.
///
/// Similar to Rust's standard library `scan` method. It applies a function to each element of the stream,
/// maintaining an internal state (accumulator) and emitting the values returned by the function.
/// The function can return `None` to terminate the stream early.
///
/// > Note: The closures have access to the [`context` object](surface_flows.mdx#the-context-object).
///
/// `scan` can also be provided with one generic lifetime persistence argument, either
/// `'tick` or `'static`, to specify how data persists. With `'tick`, the accumulator will only be maintained
/// within the same tick. With `'static`, the accumulated value will be remembered across ticks.
/// When not explicitly specified persistence defaults to `'tick`.
///
/// ```dfir
/// // Running sum example
/// source_iter([1, 2, 3, 4])
///     -> scan::<'tick>(|| 0, |acc: &mut i32, x: i32| {
///         *acc += x;
///         Some(*acc)
///     })
///     -> assert_eq([1, 3, 6, 10]);
///
/// // Early termination example
/// source_iter([1, 2, 3, 4])
///     -> scan::<'tick>(|| 1, |state: &mut i32, x: i32| {
///         *state = *state * x;
///         if *state > 6 {
///             None
///         } else {
///             Some(-*state)
///         }
///     })
///     -> assert_eq([-1, -2, -6]);
/// ```
pub const SCAN: OperatorConstraints = OperatorConstraints {
    name: "scan",
    categories: &[OperatorCategory::Fold],
    hard_range_inn: RANGE_1,
    soft_range_inn: RANGE_1,
    hard_range_out: RANGE_1,
    soft_range_out: RANGE_1,
    num_args: 2,
    persistence_args: &(0..=1),
    type_args: RANGE_0,
    is_external_input: false,
    flo_type: None,
    ports_inn: None,
    ports_out: None,
    input_delaytype_fn: |_| None,
    write_fn: |wc @ &WriteContextArgs {
                   root,
                   op_span,
                   ident,
                   is_pull,
                   inputs,
                   outputs,
                   arguments,
                   df_ident,
                   node_id,
                   ..
               },
               diagnostics| {
        let init_fn = &arguments[0];
        let func = &arguments[1];
        let singleton_output_ident = wc.make_ident("singleton_output");

        let initializer_func_ident = wc.make_ident("initializer_func");
        let init = quote_spanned! {op_span=>
            (#initializer_func_ident)()
        };

        let [persistence] = wc.persistence_args(diagnostics);

        let input = &inputs[0];
        let iterator_item_ident = wc.make_ident("iterator_item");
        let result_ident = wc.make_ident("result");
        let state_ident = wc.make_ident("scan_state");

        let redis_client_ident = wc.make_ident("redis_client");
        let redis_key_ident = wc.make_ident("redis_key");
        let init_serialized_ident = wc.make_ident("init_serialized");
        let redis_init_once_ident = wc.make_ident("redis_init_once");
        let redis_state_type_ident = wc.make_ident("redis_state_type");
        let redis_record_type_ident = wc.make_ident("redis_record_type");

        let redis_key = format!("hydro:scan:{}:{:?}", df_ident, node_id);

        let write_prologue = quote_spanned! {op_span=>
            // Represents the state of the scan.
            #[derive(#root::serde::Serialize, #root::serde::Deserialize)]
            enum #redis_state_type_ident<A> {
                Active(A),
                Terminated,
            }

            // Include a revision to distinguish individual updates.
            #[derive(#root::serde::Serialize, #root::serde::Deserialize)]
            struct #redis_record_type_ident<A> {
                revision: u64,
                state: #redis_state_type_ident<A>,
            }

             #[allow(unused_mut, reason = "for if `Fn` instead of `FnMut`.")]
            let mut #initializer_func_ident = std::sync::Arc::new(#init_fn);

            // We initialize only on the first call of write_iterator, as we cannot have blocking calls here
            // let initVal = #init;

            // let #init_serialized_ident = serde_json::to_string(&initVal).expect("Failed to serialize scan accumulator");

            let redis_url = "redis://redis.default.svc.cluster.local:6379/".to_string();

            let #redis_client_ident = #root::redis::Client::open(redis_url).expect("Invalid Redis URL");

            let #redis_key_ident = #redis_key.to_string();

            let #redis_init_once_ident = std::sync::Arc::new(#root::tokio::sync::OnceCell::<()>::new());

            //
            // #[allow(unused_mut, reason = "for if `Fn` instead of `FnMut`.")]
            // let mut #initializer_func_ident = #init_fn;
            //
            // #[allow(clippy::redundant_closure_call)]
            // let mut #singleton_output_ident = Some(#init);
        };

        let write_tick_end = match persistence {
            super::Persistence::Tick => panic!("Do not support tick persistence for scan in serverless environment at the moment"),
            _ => Default::default(),
        };

        let assign_accum_ident = quote_spanned! {op_span=>
            let #state_ident = &mut #singleton_output_ident;

            if #state_ident.is_none() {
                return None;
            }
        };

        let iterator_foreach = quote_spanned! {op_span=>
            #[inline(always)]
            fn call_scan_fn<Accum, Item, Output>(
                accum: &mut Option<Accum>,
                item: Item,
                func: impl Fn(&mut Accum, Item) -> Option<Output>,
            ) -> Option<Output> {
                let result = (func)(accum.as_mut().unwrap(), item);

                if result.is_none() {
                    *accum = None;
                }

                result
            }

            #[allow(clippy::redundant_closure_call)]
            let #result_ident = call_scan_fn(&mut *#state_ident, #iterator_item_ident, #func);
        };

        let filter_map_async_body = quote_spanned! {op_span=>
            #[inline(always)]
            async fn call_scan_redis_fn<Accum, Item, Output, Init>(
                client: #root::redis::Client,
                key: String,
                init_once: std::sync::Arc<#root::tokio::sync::OnceCell<()>>,
                init_fn: std::sync::Arc<Init>,
                item: Item,
                func: impl Fn(&mut Accum, Item) -> Option<Output>,
            ) -> Option<Output>
            where
                Accum: #root::serde::Serialize + #root::serde::de::DeserializeOwned,
                Item: Clone,
                Init: Fn() -> Accum,
            {
                init_once.get_or_try_init(|| async {
                    let initial_value: Accum = init_fn(); // note that we do not consider async initialize or step functions

                    let record = #redis_record_type_ident {
                        revision: 0,
                        state: #redis_state_type_ident::Active(initial_value),
                    };

                    let serialized: Vec<u8> = #root::serde_json::to_vec(&record).expect("Failed to serialize scan accumulator");

                    let mut conn = client.get_multiplexed_async_connection().await?;

                    let initialized: Option<String> = #root::redis::cmd("SET").arg(&key).arg(&serialized).arg("NX").query_async(&mut conn).await?;

                    println!("Initialized key {}: {}", key, initialized.is_some());

                    Ok::<(), #root::redis::RedisError>(())
                }).await.expect("Redis scan initialization failed");

                println!("Here we would actually do the next step, which is not implemented yet");

                let mut conn = client.get_multiplexed_async_connection().await.expect("Redis connection failed");

                loop {
                    let bytes_wrapped: Option<Vec<u8>> = #root::redis::cmd("GET").arg(&key).query_async(&mut conn).await.unwrap_or_else(|err| {
                        panic!("Redis GET failed for key {}: {}", key, err)
                    });

                    let bytes: Vec<u8> = bytes_wrapped.unwrap_or_else(|| {
                        panic!("Redis key {} not found, must have been deleted while still in use", key);
                    });

                    let mut record: #redis_record_type_ident<Accum> = #root::serde_json::from_slice(&bytes).expect("Failed to deserialize Redis accumulator");

                    match &mut record.state {
                        #redis_state_type_ident::Active(accumulator) => {
                            let next_val: Option<Output> = func(accumulator, item.clone());
                            if (next_val.is_none()) {
                                // terminate stream
                                record.state = #redis_state_type_ident::Terminated;
                            } // otherwise the accumulator is modified directly
                            record.revision += 1;
                            let replacement: Vec<u8> = #root::serde_json::to_vec(&record).expect("Failed to serialize incremented accumulator");

                            let result: Option<String> = #root::redis::cmd("SET").arg(&key).arg(&replacement).arg("IFEQ").arg(&bytes).query_async(&mut conn).await.expect("Redis CAS failed");

                            if (result.is_some()) {
                                return next_val; // CAS succeeded
                            }
                            // Loop again because CAS failed
                        }
                        #redis_state_type_ident::Terminated => {
                            // Scan already terminated
                            return None;
                        }
                    }
                }

                // TODO: IMPLEMENT THE ACTUAL UPDATE STEP IN A LOOP
                None
            }

            // let #state_ident = &mut #singleton_output_ident;

            #[allow(clippy::redundant_closure_call)]
            call_scan_redis_fn(#redis_client_ident.clone(), #redis_key_ident.clone(), #redis_init_once_ident.clone(), #initializer_func_ident.clone(), #iterator_item_ident, #func)
        };

        let write_iterator = if is_pull {
            quote_spanned! {op_span=>
                let #ident = #root::dfir_pipes::pull::Pull::filter_map_async(#input, |#iterator_item_ident| {
                    #filter_map_async_body
                });
            }
        } else {
            let output = &outputs[0];
            quote_spanned! {op_span=>
                let #ident = #root::dfir_pipes::push::filter_map_async(|#iterator_item_ident| {
                    #filter_map_async_body
                }, #output);
            }
        };

        Ok(OperatorWriteOutput {
            write_prologue,
            write_iterator,
            write_tick_end,
            ..Default::default()
        })
    },
};
