use rocket::State;
use rocket::request::Form;
use rocket_contrib::json::Json;
use serde_json::Value;

use crate::CONFIG;
use crate::db::DbConn;
use crate::db::models::*;

use crate::api::{PasswordData, JsonResult, EmptyResult, NumberOrString, JsonUpcase, WebSocketUsers, UpdateType};
use crate::auth::{Headers, AdminHeaders, OwnerHeaders};

use serde::{Deserialize, Deserializer};

use rocket::Route;

pub fn routes() -> Vec<Route> {
    routes![
        get_organization,
        create_organization,
        delete_organization,
        post_delete_organization,
        leave_organization,
        get_user_collections,
        get_org_collections,
        get_org_collection_detail,
        get_collection_users,
        put_organization,
        post_organization,
        post_organization_collections,
        delete_organization_collection_user,
        post_organization_collection_delete_user,
        post_organization_collection_update,
        put_organization_collection_update,
        delete_organization_collection,
        post_organization_collection_delete,
        get_org_details,
        get_org_users,
        send_invite,
        confirm_invite,
        get_user,
        edit_user,
        put_organization_user,
        delete_user,
        post_delete_user,
        post_reinvite_user,
        post_org_import,
    ]
}


#[derive(Deserialize)]
#[allow(non_snake_case)]
struct OrgData {
    BillingEmail: String,
    CollectionName: String,
    Key: String,
    Name: String,
    #[serde(rename = "PlanType")]
    _PlanType: NumberOrString, // Ignored, always use the same plan
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct OrganizationUpdateData {
    BillingEmail: String,
    Name: String,
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct NewCollectionData {
    Name: String,
}

#[post("/organizations", data = "<data>")]
fn create_organization(headers: Headers, data: JsonUpcase<OrgData>, conn: DbConn) -> JsonResult {
    let data: OrgData = data.into_inner().data;

    let mut org = Organization::new(data.Name, data.BillingEmail);
    let mut user_org = UserOrganization::new(
        headers.user.uuid.clone(), org.uuid.clone());
    let mut collection = Collection::new(
        org.uuid.clone(), data.CollectionName);

    user_org.key = data.Key;
    user_org.access_all = true;
    user_org.type_ = UserOrgType::Owner as i32;
    user_org.status = UserOrgStatus::Confirmed as i32;

    org.save(&conn)?;
    user_org.save(&conn)?;
    collection.save(&conn)?;

    Ok(Json(org.to_json()))
}

#[delete("/organizations/<org_id>", data = "<data>")]
fn delete_organization(org_id: String, data: JsonUpcase<PasswordData>, headers: OwnerHeaders, conn: DbConn) -> EmptyResult {
    let data: PasswordData = data.into_inner().data;
    let password_hash = data.MasterPasswordHash;

    if !headers.user.check_valid_password(&password_hash) {
        err!("Invalid password")
    }

    match Organization::find_by_uuid(&org_id, &conn) {
        None => err!("Organization not found"),
        Some(org) => org.delete(&conn)
    }
}

#[post("/organizations/<org_id>/delete", data = "<data>")]
fn post_delete_organization(org_id: String, data: JsonUpcase<PasswordData>, headers: OwnerHeaders, conn: DbConn) -> EmptyResult {
    delete_organization(org_id, data, headers, conn)
}

#[post("/organizations/<org_id>/leave")]
fn leave_organization(org_id: String, headers: Headers, conn: DbConn) -> EmptyResult {
    match UserOrganization::find_by_user_and_org(&headers.user.uuid, &org_id, &conn) {
        None => err!("User not part of organization"),
        Some(user_org) => {
            if user_org.type_ == UserOrgType::Owner {
                let num_owners = UserOrganization::find_by_org_and_type(
                    &org_id, UserOrgType::Owner as i32, &conn)
                    .len();

                if num_owners <= 1 {
                    err!("The last owner can't leave")
                }
            }
            
            user_org.delete(&conn)
        }
    }
}

#[get("/organizations/<org_id>")]
fn get_organization(org_id: String, _headers: OwnerHeaders, conn: DbConn) -> JsonResult {
    match Organization::find_by_uuid(&org_id, &conn) {
        Some(organization) => Ok(Json(organization.to_json())),
        None => err!("Can't find organization details")
    }
}

#[put("/organizations/<org_id>", data = "<data>")]
fn put_organization(org_id: String, headers: OwnerHeaders, data: JsonUpcase<OrganizationUpdateData>, conn: DbConn) -> JsonResult {
    post_organization(org_id, headers, data, conn)
}

#[post("/organizations/<org_id>", data = "<data>")]
fn post_organization(org_id: String, _headers: OwnerHeaders, data: JsonUpcase<OrganizationUpdateData>, conn: DbConn) -> JsonResult {
    let data: OrganizationUpdateData = data.into_inner().data;

    let mut org = match Organization::find_by_uuid(&org_id, &conn) {
        Some(organization) => organization,
        None => err!("Can't find organization details")
    };

    org.name = data.Name;
    org.billing_email = data.BillingEmail;

    org.save(&conn)?;
    Ok(Json(org.to_json()))
}

// GET /api/collections?writeOnly=false
#[get("/collections")]
fn get_user_collections(headers: Headers, conn: DbConn) -> JsonResult {

    Ok(Json(json!({
        "Data":
            Collection::find_by_user_uuid(&headers.user.uuid, &conn)
            .iter()
            .map(Collection::to_json)
            .collect::<Value>(),
        "Object": "list",
        "ContinuationToken": null,
    })))
}

#[get("/organizations/<org_id>/collections")]
fn get_org_collections(org_id: String, _headers: AdminHeaders, conn: DbConn) -> JsonResult {
    Ok(Json(json!({
        "Data":
            Collection::find_by_organization(&org_id, &conn)
            .iter()
            .map(Collection::to_json)
            .collect::<Value>(),
        "Object": "list",
        "ContinuationToken": null,
    })))
}

#[post("/organizations/<org_id>/collections", data = "<data>")]
fn post_organization_collections(org_id: String, _headers: AdminHeaders, data: JsonUpcase<NewCollectionData>, conn: DbConn) -> JsonResult {
    let data: NewCollectionData = data.into_inner().data;

    let org = match Organization::find_by_uuid(&org_id, &conn) {
        Some(organization) => organization,
        None => err!("Can't find organization details")
    };

    let mut collection = Collection::new(org.uuid.clone(), data.Name);
    collection.save(&conn)?;

    Ok(Json(collection.to_json()))
}

#[put("/organizations/<org_id>/collections/<col_id>", data = "<data>")]
fn put_organization_collection_update(org_id: String, col_id: String, headers: AdminHeaders, data: JsonUpcase<NewCollectionData>, conn: DbConn) -> JsonResult {
    post_organization_collection_update(org_id, col_id, headers, data, conn)
}

#[post("/organizations/<org_id>/collections/<col_id>", data = "<data>")]
fn post_organization_collection_update(org_id: String, col_id: String, _headers: AdminHeaders, data: JsonUpcase<NewCollectionData>, conn: DbConn) -> JsonResult {
    let data: NewCollectionData = data.into_inner().data;

    let org = match Organization::find_by_uuid(&org_id, &conn) {
        Some(organization) => organization,
        None => err!("Can't find organization details")
    };

    let mut collection = match Collection::find_by_uuid(&col_id, &conn) {
        Some(collection) => collection,
        None => err!("Collection not found")
    };

    if collection.org_uuid != org.uuid {
        err!("Collection is not owned by organization");
    }

    collection.name = data.Name.clone();
    collection.save(&conn)?;

    Ok(Json(collection.to_json()))
}


#[delete("/organizations/<org_id>/collections/<col_id>/user/<org_user_id>")]
fn delete_organization_collection_user(org_id: String, col_id: String, org_user_id: String, _headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let collection = match Collection::find_by_uuid(&col_id, &conn) {
        None => err!("Collection not found"),
        Some(collection) => if collection.org_uuid == org_id {
            collection
        } else {
            err!("Collection and Organization id do not match")
        }
    };

    match UserOrganization::find_by_uuid_and_org(&org_user_id, &org_id, &conn) {
        None => err!("User not found in organization"),
        Some(user_org) => {
            match CollectionUser::find_by_collection_and_user(&collection.uuid, &user_org.user_uuid, &conn) {
                None => err!("User not assigned to collection"),
                Some(col_user) => {
                    col_user.delete(&conn)
                }
            }
        }
    }
}

#[post("/organizations/<org_id>/collections/<col_id>/delete-user/<org_user_id>")]
fn post_organization_collection_delete_user(org_id: String, col_id: String, org_user_id: String, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    delete_organization_collection_user(org_id, col_id, org_user_id, headers, conn)
}

#[delete("/organizations/<org_id>/collections/<col_id>")]
fn delete_organization_collection(org_id: String, col_id: String, _headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    match Collection::find_by_uuid(&col_id, &conn) {
        None => err!("Collection not found"),
        Some(collection) => if collection.org_uuid == org_id {
            collection.delete(&conn)
        } else {
            err!("Collection and Organization id do not match")
        }
    }
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct DeleteCollectionData {
    Id: String,
    OrgId: String,
}

#[post("/organizations/<org_id>/collections/<col_id>/delete", data = "<_data>")]
fn post_organization_collection_delete(org_id: String, col_id: String, headers: AdminHeaders, _data: JsonUpcase<DeleteCollectionData>, conn: DbConn) -> EmptyResult {
    delete_organization_collection(org_id, col_id, headers, conn)
}

#[get("/organizations/<org_id>/collections/<coll_id>/details")]
fn get_org_collection_detail(org_id: String, coll_id: String, headers: AdminHeaders, conn: DbConn) -> JsonResult {
    match Collection::find_by_uuid_and_user(&coll_id, &headers.user.uuid, &conn) {
        None => err!("Collection not found"),
        Some(collection) => {
            if collection.org_uuid != org_id {
                err!("Collection is not owned by organization")
            }

            Ok(Json(collection.to_json()))
        }
    }
}

#[get("/organizations/<org_id>/collections/<coll_id>/users")]
fn get_collection_users(org_id: String, coll_id: String, _headers: AdminHeaders, conn: DbConn) -> JsonResult {
    // Get org and collection, check that collection is from org
    let collection = match Collection::find_by_uuid_and_org(&coll_id, &org_id, &conn) {
        None => err!("Collection not found in Organization"),
        Some(collection) => collection
    };

    // Get the users from collection
    let user_list: Vec<Value> = CollectionUser::find_by_collection(&collection.uuid, &conn)
    .iter().map(|col_user|  {
        UserOrganization::find_by_user_and_org(&col_user.user_uuid, &org_id, &conn)
        .unwrap()
        .to_json_collection_user_details(col_user.read_only, &conn)
    }).collect();

    Ok(Json(json!({
        "Data": user_list,
        "Object": "list",
        "ContinuationToken": null,
    })))
}

#[derive(FromForm)]
struct OrgIdData {
    #[form(field = "organizationId")]
    organization_id: String
}

#[get("/ciphers/organization-details?<data..>")]
fn get_org_details(data: Form<OrgIdData>, headers: Headers, conn: DbConn) -> JsonResult {
    let ciphers = Cipher::find_by_org(&data.organization_id, &conn);
    let ciphers_json: Vec<Value> = ciphers.iter().map(|c| c.to_json(&headers.host, &headers.user.uuid, &conn)).collect();

    Ok(Json(json!({
      "Data": ciphers_json,
      "Object": "list",
      "ContinuationToken": null,
    })))
}

#[get("/organizations/<org_id>/users")]
fn get_org_users(org_id: String, _headers: AdminHeaders, conn: DbConn) -> JsonResult {
    let users = UserOrganization::find_by_org(&org_id, &conn);
    let users_json: Vec<Value> = users.iter().map(|c| c.to_json_user_details(&conn)).collect();

    Ok(Json(json!({
        "Data": users_json,
        "Object": "list",
        "ContinuationToken": null,
    })))
}

fn deserialize_collections<'de, D>(deserializer: D) -> Result<Vec<CollectionData>, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserialize null to empty Vec
    Deserialize::deserialize(deserializer).or(Ok(vec![])) 
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct CollectionData {
    Id: String,
    ReadOnly: bool,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct InviteData {
    Emails: Vec<String>,
    Type: NumberOrString,
    #[serde(deserialize_with = "deserialize_collections")]
    Collections: Vec<CollectionData>,
    AccessAll: Option<bool>,
}

#[post("/organizations/<org_id>/users/invite", data = "<data>")]
fn send_invite(org_id: String, data: JsonUpcase<InviteData>, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let data: InviteData = data.into_inner().data;

    let new_type = match UserOrgType::from_str(&data.Type.into_string()) {
        Some(new_type) => new_type as i32,
        None => err!("Invalid type")
    };

    if new_type != UserOrgType::User &&
        headers.org_user_type != UserOrgType::Owner {
        err!("Only Owners can invite Managers, Admins or Owners")
    }

    for email in data.Emails.iter() {
        let mut user_org_status = UserOrgStatus::Accepted as i32;
        let user = match User::find_by_mail(&email, &conn) {
            None => if CONFIG.invitations_allowed { // Invite user if that's enabled
                let mut invitation = Invitation::new(email.clone());
                invitation.save(&conn)?;
                let mut user = User::new(email.clone());
                user.save(&conn)?;
                user_org_status = UserOrgStatus::Invited as i32;
                user
                
            } else {
                err!(format!("User email does not exist: {}", email))
            },
            Some(user) => if UserOrganization::find_by_user_and_org(&user.uuid, &org_id, &conn).is_some() {
                err!(format!("User already in organization: {}", email))
            } else {
                user
            }

        };

        let mut new_user = UserOrganization::new(user.uuid.clone(), org_id.clone());
        let access_all = data.AccessAll.unwrap_or(false);
        new_user.access_all = access_all;
        new_user.type_ = new_type;
        new_user.status = user_org_status;

        // If no accessAll, add the collections received
        if !access_all {
            for col in &data.Collections {
                match Collection::find_by_uuid_and_org(&col.Id, &org_id, &conn) {
                    None => err!("Collection not found in Organization"),
                    Some(collection) => {
                        CollectionUser::save(&user.uuid, &collection.uuid, col.ReadOnly, &conn)?;
                    }
                }
            }
        }

        new_user.save(&conn)?;
    }

    Ok(())
}

#[post("/organizations/<org_id>/users/<org_user_id>/confirm", data = "<data>")]
fn confirm_invite(org_id: String, org_user_id: String, data: JsonUpcase<Value>, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let data = data.into_inner().data;

    let mut user_to_confirm = match UserOrganization::find_by_uuid_and_org(&org_user_id, &org_id, &conn) {
        Some(user) => user,
        None => err!("The specified user isn't a member of the organization")
    };

    if user_to_confirm.type_ != UserOrgType::User &&
        headers.org_user_type != UserOrgType::Owner {
        err!("Only Owners can confirm Managers, Admins or Owners")
    }

    if user_to_confirm.status != UserOrgStatus::Accepted as i32 {
        err!("User in invalid state")
    }

    user_to_confirm.status = UserOrgStatus::Confirmed as i32;
    user_to_confirm.key = match data["Key"].as_str() {
        Some(key) => key.to_string(),
        None => err!("Invalid key provided")
    };

    user_to_confirm.save(&conn)
}

#[get("/organizations/<org_id>/users/<org_user_id>")]
fn get_user(org_id: String, org_user_id: String, _headers: AdminHeaders, conn: DbConn) -> JsonResult {
    let user = match UserOrganization::find_by_uuid_and_org(&org_user_id, &org_id, &conn) {
        Some(user) => user,
        None => err!("The specified user isn't a member of the organization")
    };

    Ok(Json(user.to_json_details(&conn)))
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct EditUserData {
    Type: NumberOrString,
    #[serde(deserialize_with = "deserialize_collections")]
    Collections: Vec<CollectionData>,
    AccessAll: bool,
}

#[put("/organizations/<org_id>/users/<org_user_id>", data = "<data>", rank = 1)]
fn put_organization_user(org_id: String, org_user_id: String, data: JsonUpcase<EditUserData>, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    edit_user(org_id, org_user_id, data, headers, conn)
}

#[post("/organizations/<org_id>/users/<org_user_id>", data = "<data>", rank = 1)]
fn edit_user(org_id: String, org_user_id: String, data: JsonUpcase<EditUserData>, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let data: EditUserData = data.into_inner().data;

    let new_type = match UserOrgType::from_str(&data.Type.into_string()) {
        Some(new_type) => new_type,
        None => err!("Invalid type")
    };

    let mut user_to_edit = match UserOrganization::find_by_uuid_and_org(&org_user_id, &org_id, &conn) {
        Some(user) => user,
        None => err!("The specified user isn't member of the organization")
    };

    if new_type != user_to_edit.type_ && (
            user_to_edit.type_ >= UserOrgType::Admin ||
            new_type >= UserOrgType::Admin
        ) &&
        headers.org_user_type != UserOrgType::Owner {
        err!("Only Owners can grant and remove Admin or Owner privileges")
    }

    if user_to_edit.type_ == UserOrgType::Owner &&
        headers.org_user_type != UserOrgType::Owner {
        err!("Only Owners can edit Owner users")
    }

    if user_to_edit.type_ == UserOrgType::Owner &&
        new_type != UserOrgType::Owner {

        // Removing owner permmission, check that there are at least another owner
        let num_owners = UserOrganization::find_by_org_and_type(
            &org_id, UserOrgType::Owner as i32, &conn)
            .len();

        if num_owners <= 1 {
            err!("Can't delete the last owner")
        }
    }

    user_to_edit.access_all = data.AccessAll;
    user_to_edit.type_ = new_type as i32;

    // Delete all the odd collections
    for c in CollectionUser::find_by_organization_and_user_uuid(&org_id, &user_to_edit.user_uuid, &conn) {
        c.delete(&conn)?;
    }

    // If no accessAll, add the collections received
    if !data.AccessAll {
        for col in &data.Collections {
            match Collection::find_by_uuid_and_org(&col.Id, &org_id, &conn) {
                None => err!("Collection not found in Organization"),
                Some(collection) => {
                    CollectionUser::save(&user_to_edit.user_uuid, &collection.uuid, col.ReadOnly, &conn)?;
                }
            }
        }
    }

    user_to_edit.save(&conn)
}

#[delete("/organizations/<org_id>/users/<org_user_id>")]
fn delete_user(org_id: String, org_user_id: String, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let user_to_delete = match UserOrganization::find_by_uuid_and_org(&org_user_id, &org_id, &conn) {
        Some(user) => user,
        None => err!("User to delete isn't member of the organization")
    };

    if user_to_delete.type_ != UserOrgType::User &&
        headers.org_user_type != UserOrgType::Owner {
        err!("Only Owners can delete Admins or Owners")
    }

    if user_to_delete.type_ == UserOrgType::Owner {
        // Removing owner, check that there are at least another owner
        let num_owners = UserOrganization::find_by_org_and_type(
            &org_id, UserOrgType::Owner as i32, &conn)
            .len();

        if num_owners <= 1 {
            err!("Can't delete the last owner")
        }
    }

    user_to_delete.delete(&conn)
}

#[post("/organizations/<org_id>/users/<org_user_id>/delete")]
fn post_delete_user(org_id: String, org_user_id: String, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    delete_user(org_id, org_user_id, headers, conn)
}

#[post("/organizations/<_org_id>/users/<_org_user_id>/reinvite")]
fn post_reinvite_user(_org_id: String, _org_user_id: String, _headers: AdminHeaders, _conn: DbConn) -> EmptyResult {
    err!("This functionality is not implemented. The user needs to manually register before they can be accepted into the organization.")
}

use super::ciphers::CipherData;
use super::ciphers::update_cipher_from_data;

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct ImportData {
    Ciphers: Vec<CipherData>,
    Collections: Vec<NewCollectionData>,
    CollectionRelationships: Vec<RelationsData>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct RelationsData {
    // Cipher index
    Key: usize,
    // Collection index
    Value: usize,
}

#[post("/ciphers/import-organization?<query..>", data = "<data>")]
fn post_org_import(query: Form<OrgIdData>, data: JsonUpcase<ImportData>, headers: Headers, conn: DbConn, ws: State<WebSocketUsers>) -> EmptyResult {
    let data: ImportData = data.into_inner().data;
    let org_id = query.into_inner().organization_id;

    let org_user = match UserOrganization::find_by_user_and_org(&headers.user.uuid, &org_id, &conn) {
        Some(user) => user,
        None => err!("User is not part of the organization")
    };

    if org_user.type_ < UserOrgType::Admin {
        err!("Only admins or owners can import into an organization")
    }

    // Read and create the collections
    let collections: Vec<_> = data.Collections.into_iter().map(|coll| {
        let mut collection = Collection::new(org_id.clone(), coll.Name);
        if collection.save(&conn).is_err() {
            err!("Failed to create Collection");
        }
        
        Ok(collection)
    }).collect();

    // Read the relations between collections and ciphers
    let mut relations = Vec::new();
    for relation in data.CollectionRelationships {
        relations.push((relation.Key, relation.Value));
    }

    // Read and create the ciphers
    let ciphers: Vec<_> = data.Ciphers.into_iter().map(|cipher_data| {
        let mut cipher = Cipher::new(cipher_data.Type, cipher_data.Name.clone());
        update_cipher_from_data(&mut cipher, cipher_data, &headers, false, &conn, &ws, UpdateType::SyncCipherCreate).ok();
        cipher
    }).collect();

    // Assign the collections
    for (cipher_index, coll_index) in relations {
        let cipher_id = &ciphers[cipher_index].uuid;
        let coll = &collections[coll_index];
        let coll_id = match coll {
            Ok(coll) => coll.uuid.as_str(),
            Err(_) => err!("Failed to assign to collection")
        };
        
        CollectionCipher::save(cipher_id, coll_id, &conn)?;
    }

    let mut user = headers.user;
    user.update_revision(&conn)
}
