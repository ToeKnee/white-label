use leptos::prelude::*;
use server_fn::codec::{MultipartData, MultipartFormData, StreamingText, TextStream};
#[cfg(feature = "ssr")]
use std::{fs::File, io::Write};

#[cfg(feature = "ssr")]
use crate::config::upload::{UploadConfiguration, UploadDetails};
#[cfg(feature = "ssr")]
use crate::models::auth::User;
#[cfg(feature = "ssr")]
use crate::services::{
    authentication_helpers::user_with_permissions,
    files::progress::{FILES, add_chunk, progress_for_file},
};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool, user_context};
#[cfg(feature = "ssr")]
use crate::utils::files::valid_file_name;

/// Get the configuration upload details.
///
/// This function will return the upload details for the given configuration.
///
/// Errors:
/// - Invalid upload configuration
#[cfg(feature = "ssr")]
fn upload_details(config_str: &str) -> Result<UploadDetails, ServerFnError> {
    let upload_config = match config_str.parse::<UploadConfiguration>() {
        Ok(config) => config,
        Err(e) => {
            tracing::error!("Invalid upload configuration: {e}");
            return Err(ServerFnError::new(
                "Invalid upload configuration.".to_string(),
            ));
        }
    };

    let details = upload_config.get_details();
    Ok(details)
}

/// Upload a file.
/// This function will receive a multipart form data request and save the file to disk.
#[allow(clippy::too_many_lines)]
#[server(UploadFile, "/api", endpoint="upload_file", input = MultipartFormData,)]
pub async fn upload_file(data: MultipartData) -> Result<(), ServerFnError> {
    tracing::warn!("TODO: Error handling");

    let Some(mut data) = data.into_inner() else {
        return Err(ServerFnError::new("No data in request.".to_string()));
    };

    // The order of fields is important
    // The first field should be named "type" and should contain the upload configuration type
    // The second field should be the file to upload
    let type_field = match data.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => {
            return Err(ServerFnError::new("No fields in request.".to_string()));
        }
        Err(e) => {
            return Err(ServerFnError::new(e.to_string()));
        }
    };
    let upload_config_type = match type_field.text().await {
        Ok(config) => config,
        Err(e) => {
            return Err(ServerFnError::new(e.to_string()));
        }
    };

    let upload_details = match upload_details(&upload_config_type) {
        Ok(details) => details,
        Err(e) => return Err(ServerFnError::new(e)),
    };

    // Get the `slug` or other identifier for the row we are storing the file for
    let slug_field = match data.next_field().await {
        Ok(Some(field)) => match field.text().await {
            Ok(slug) => slug,
            Err(e) => {
                return Err(ServerFnError::new(e.to_string()));
            }
        },
        Ok(None) => {
            return Err(ServerFnError::new("No fields in request.".to_string()));
        }
        Err(e) => {
            return Err(ServerFnError::new(e.to_string()));
        }
    };

    let auth = auth()?;
    // Convert vector of String to vector of &str
    let permissions = upload_details
        .permissions
        .iter()
        .map(std::string::String::as_str)
        .collect();
    let user = match user_with_permissions(auth.current_user.as_ref(), permissions) {
        Ok(user) => user,
        Err(e) => return Err(ServerFnError::new(e)),
    };

    let mut more = true;
    while more {
        match data.next_field().await {
            Ok(None) => {
                more = false;
            }
            Ok(Some(mut field)) => {
                // Check the content type of the field.
                match field.content_type() {
                    Some(content_type) => {
                        if !upload_details
                            .mime_types
                            .contains(&content_type.to_string())
                        {
                            return Err(ServerFnError::new("Invalid mime type.".to_string()));
                        }
                    }
                    None => {
                        return Err(ServerFnError::new("No content type on field.".to_string()));
                    }
                };

                let original_file_name = match field.file_name() {
                    Some(file_name) => file_name.to_string(),
                    None => {
                        return Err(ServerFnError::new("No filename on field.".to_string()));
                    }
                };

                let file_name = match valid_file_name(
                    &original_file_name,
                    Some(user.username.clone()),
                    &upload_details.path,
                    true,
                ) {
                    Ok(name) => name,
                    Err(e) => return Err(e),
                };

                let Ok(upload_path) = std::env::var("UPLOAD_PATH") else {
                    return Err(ServerFnError::new("No upload path specified.".to_string()));
                };

                let tmp_path = format!("{upload_path}/tmp/{file_name}");
                tracing::info!("Uploading {file_name} to {tmp_path}");
                let mut f = File::create(tmp_path.clone())?;
                let mut chunk_more = true;
                while chunk_more {
                    match field.chunk().await {
                        Ok(None) => {
                            // File upload complete
                            chunk_more = false;
                            match finalise_file_upload(
                                upload_config_type.clone(),
                                file_name.clone(),
                                original_file_name.clone(),
                                slug_field.clone(),
                                user.clone(),
                            )
                            .await
                            {
                                Ok(()) => (),
                                Err(e) => return Err(e),
                            }
                        }
                        Ok(Some(chunk)) => {
                            let len = chunk.len();
                            let total_so_far =
                                add_chunk(&original_file_name, len, &user.username).await;

                            if total_so_far > upload_details.size_limit {
                                // Delete file and return error
                                std::fs::remove_file(tmp_path.clone())?;
                                let id = format!("{}-{original_file_name}", user.username);
                                FILES.remove(&id);
                                return Err(ServerFnError::new("File too large.".to_string()));
                            }

                            f.write_all(&chunk)?;
                        }
                        Err(e) => {
                            return Err(ServerFnError::new(e.to_string()));
                        }
                    }
                }
            }
            Err(e) => {
                return Err(ServerFnError::new(e.to_string()));
            }
        }
    }

    Ok(())
}

/// Finalise the file upload.
/// This function will move the file from the temporary location to the final location.
/// It will also associate the file with the object.
///
/// # Errors
/// - No upload path specified
/// - Unable to move the file
/// - Unable to associate the file with the object
#[cfg(feature = "ssr")]
async fn finalise_file_upload(
    upload_config_type: String,
    file_name: String,
    original_file_name: String,
    slug_field: String,
    user: User,
) -> Result<(), ServerFnError> {
    let Ok(upload_path) = std::env::var("UPLOAD_PATH") else {
        return Err(ServerFnError::new("No upload path specified.".to_string()));
    };
    let tmp_path = format!("{upload_path}/tmp/{file_name}");

    let upload_details = match upload_details(&upload_config_type) {
        Ok(details) => details,
        Err(e) => return Err(ServerFnError::new(e)),
    };

    let path = format!("{upload_path}/{}/{file_name}", upload_details.path);
    match std::fs::rename(tmp_path.clone(), path) {
        Ok(()) => {
            tracing::info!("File uploaded.");
            let id = format!("{}-{original_file_name}", user.username);
            FILES.remove(&id);
        }
        Err(e) => {
            return Err(ServerFnError::new(e.to_string()));
        }
    }
    // Associate the file with the object.
    let _ = store_file_to_object(&file_name, &upload_config_type, &slug_field).await;

    Ok(())
}

/// Store the file to the identified object.
/// This function will store the file name the database.
///
/// # Errors
/// - No user found in request
/// - Unable to get the user from the database
/// - Unable to to update the user
#[cfg(feature = "ssr")]
async fn store_file_to_object(
    file_name: &str,
    upload_config_type: &str,
    slug_field: &str,
) -> Result<(), ServerFnError> {
    let mut auth = auth()?;
    let pool = pool()?;

    if upload_config_type == "Avatar" {
        let Some(current_user) = auth.current_user.as_ref() else {
            return Err(ServerFnError::new("No user found.".to_string()));
        };
        if slug_field == current_user.username {
            // Store the file to the user
            let mut user = match User::get_by_username(&pool, slug_field.to_string()).await {
                Ok(user) => user,
                Err(e) => {
                    tracing::error!("Couldn't get user: {e}");
                    return Err(ServerFnError::new(e));
                }
            };
            user.avatar = Some(file_name.to_string());
            match user.update(&pool).await {
                Ok(user) => {
                    auth.reload_user().await;
                    let user_context = user_context()?;
                    user_context.1.set(user);
                }
                Err(e) => {
                    tracing::error!("Couldn't update user: {e}");
                    return Err(ServerFnError::new(e));
                }
            };
        }
    }
    Ok(())
}

/// Get the progress of a file upload.
/// This function will return a stream of the current length of the file.
/// The stream will be a series of `usize` values, each separated by a newline.
#[allow(clippy::unused_async)] // Although this function doesn't use `async`, it's required for the `#[server]` macro
#[server(FileProgress, "/api", endpoint="file_progress", output = StreamingText)]
pub async fn file_progress(filename: String) -> Result<TextStream, ServerFnError> {
    use futures::StreamExt;

    let auth = auth()?;
    let user = match user_with_permissions(auth.current_user.as_ref(), vec!["admin", "label_owner"])
    {
        Ok(user) => user,
        Err(e) => return Err(ServerFnError::new(e)),
    };

    tracing::debug!("Getting progress on {filename}");
    // Get the stream of current length for the file
    let progress = progress_for_file(&filename, &user.username);
    // Separate each number with a newline
    // the HTTP response might pack multiple lines of this into a single chunk
    // we need some way of dividing them up
    let progress = progress.map(|bytes| Ok(format!("{bytes}\n")));
    Ok(TextStream::new(progress))
}
