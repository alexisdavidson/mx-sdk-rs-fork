multiversx_sc::imports!();
multiversx_sc::derive_imports!();

const NFT_AMOUNT: u32 = 1;
const ROYALTIES_MAX: u32 = 10_000;

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct PriceTag<M: ManagedTypeApi> {
    pub token: EgldOrEsdtTokenIdentifier<M>,
    pub nonce: u64,
    pub amount: BigUint<M>,
}

#[multiversx_sc::module]
pub trait NftModule {
    // endpoints - owner-only

    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(issueToken)]
    fn issue_token(&self, token_name: ManagedBuffer, token_ticker: ManagedBuffer) {
        require!(self.nft_token_id().is_empty(), "Token already issued");

        let payment_amount = self.call_value().egld_value();
        self.send()
            .esdt_system_sc_proxy()
            .issue_non_fungible(
                payment_amount,
                &token_name,
                &token_ticker,
                NonFungibleTokenProperties {
                    can_freeze: true,
                    can_wipe: true,
                    can_pause: true,
                    can_transfer_create_role: true,
                    can_change_owner: false,
                    can_upgrade: false,
                    can_add_special_roles: true,
                },
            )
            .async_call()
            .with_callback(self.callbacks().issue_callback())
            .call_and_exit()
    }

    #[only_owner]
    #[endpoint(setLocalRoles)]
    fn set_local_roles(&self) {
        self.require_token_issued();

        self.send()
            .esdt_system_sc_proxy()
            .set_special_roles(
                &self.blockchain().get_sc_address(),
                &self.nft_token_id().get(),
                [EsdtLocalRole::NftCreate][..].iter().cloned(),
            )
            .async_call()
            .call_and_exit()
    }

    // endpoints

    #[payable("*")]
    #[endpoint(buyNft)]
    fn buy_nft(&self, nft_nonce: u64) {
        let payment = self.call_value().egld_or_single_esdt();

        self.require_token_issued();
        require!(
            !self.price_tag(nft_nonce).is_empty(),
            "Invalid nonce or NFT was already sold"
        );

        let price_tag = self.price_tag(nft_nonce).get();
        require!(
            payment.token_identifier == price_tag.token,
            "Invalid token used as payment"
        );
        require!(
            payment.token_nonce == price_tag.nonce,
            "Invalid nonce for payment token"
        );
        require!(
            payment.amount == price_tag.amount,
            "Invalid amount as payment"
        );

        self.price_tag(nft_nonce).clear();

        let nft_token_id = self.nft_token_id().get();
        let caller = self.blockchain().get_caller();
        self.send().direct_esdt(
            &caller,
            &nft_token_id,
            nft_nonce,
            &BigUint::from(NFT_AMOUNT),
        );

        let owner = self.blockchain().get_owner_address();
        self.send().direct(
            &owner,
            &payment.token_identifier,
            payment.token_nonce,
            &payment.amount,
        );
    }

    // views

    #[allow(clippy::type_complexity)]
    #[view(getNftPrice)]
    fn get_nft_price(
        &self,
        nft_nonce: u64,
    ) -> OptionalValue<MultiValue3<EgldOrEsdtTokenIdentifier, u64, BigUint>> {
        if self.price_tag(nft_nonce).is_empty() {
            // NFT was already sold
            OptionalValue::None
        } else {
            let price_tag = self.price_tag(nft_nonce).get();

            OptionalValue::Some((price_tag.token, price_tag.nonce, price_tag.amount).into())
        }
    }

    // callbacks

    #[callback]
    fn issue_callback(
        &self,
        #[call_result] result: ManagedAsyncCallResult<EgldOrEsdtTokenIdentifier>,
    ) {
        match result {
            ManagedAsyncCallResult::Ok(token_id) => {
                self.nft_token_id().set(&token_id.unwrap_esdt());
            },
            ManagedAsyncCallResult::Err(_) => {
                let caller = self.blockchain().get_owner_address();
                let returned = self.call_value().egld_or_single_esdt();
                if returned.token_identifier.is_egld() && returned.amount > 0 {
                    self.send()
                        .direct(&caller, &returned.token_identifier, 0, &returned.amount);
                }
            },
        }
    }

    // private

    #[allow(clippy::too_many_arguments)]
    fn create_nft_with_attributes<T: TopEncode>(
        &self,
        name: ManagedBuffer,
        royalties: BigUint,
        attributes: T,
        uri: ManagedBuffer,
        selling_price: BigUint,
        token_used_as_payment: EgldOrEsdtTokenIdentifier,
        token_used_as_payment_nonce: u64,
    ) -> u64 {
        self.require_token_issued();
        require!(royalties <= ROYALTIES_MAX, "Royalties cannot exceed 100%");

        let nft_token_id = self.nft_token_id().get();

        let mut serialized_attributes = ManagedBuffer::new();
        if let core::result::Result::Err(err) = attributes.top_encode(&mut serialized_attributes) {
            sc_panic!("Attributes encode error: {}", err.message_bytes());
        }

        let attributes_sha256 = self.crypto().sha256(&serialized_attributes);
        let attributes_hash = attributes_sha256.as_managed_buffer();
        let uris = ManagedVec::from_single_item(uri);
        let nft_nonce = self.send().esdt_nft_create(
            &nft_token_id,
            &BigUint::from(NFT_AMOUNT),
            &name,
            &royalties,
            attributes_hash,
            &attributes,
            &uris,
        );

        self.price_tag(nft_nonce).set(&PriceTag {
            token: token_used_as_payment,
            nonce: token_used_as_payment_nonce,
            amount: selling_price,
        });

        nft_nonce
    }

    fn require_token_issued(&self) {
        require!(!self.nft_token_id().is_empty(), "Token not issued");
    }

    // Setters
    
    // Set nft_token_id
    #[only_owner]
    #[endpoint]
    fn set_nft_token_id(&self, token_id: TokenIdentifier ) {
        self.nft_token_id().set(&token_id);
    }
    
    // Set mint_enabled
    #[only_owner]
    #[endpoint]
    fn set_mint_enabled(&self, mint_enabled: bool ) {
        self.mint_enabled().set(&mint_enabled);
    }
    
    // Set image_folder_uri
    #[only_owner]
    #[endpoint]
    fn set_image_folder_uri(&self, image_folder_uri: ManagedBuffer ) {
        self.image_folder_uri().set(&image_folder_uri);
    }

    // storage

    #[view(getMintEnabled)]
    #[storage_mapper("mintEnabled")]
    fn mint_enabled(&self) -> SingleValueMapper<bool>;

    #[view(getImageFolderUri)]
    #[storage_mapper("imageFolderUri")]
    fn image_folder_uri(&self) -> SingleValueMapper<ManagedBuffer>;

    #[view(getAttributeFolderUri)]
    #[storage_mapper("attributeFolderUri")]
    fn attribute_folder_uri(&self) -> SingleValueMapper<ManagedBuffer>;

    #[view(getNftNamePrefix)]
    #[storage_mapper("nftNamePrefix")]
    fn nft_name_prefix(&self) -> SingleValueMapper<ManagedBuffer>;

    #[view(getCollectionUri)]
    #[storage_mapper("collectionUri")]
    fn collection_uri(&self) -> SingleValueMapper<ManagedBuffer>;

    #[view(getTokenId)]
    #[storage_mapper("nftTokenId")]
    fn nft_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    #[view(getMaxSupply)]
    #[storage_mapper("maxSupply")]
    fn max_supply(&self) -> SingleValueMapper<u64>;

    #[view(getRoyalties)]
    #[storage_mapper("royalties")]
    fn royalties(&self) -> SingleValueMapper<u64>;

    #[view(getWhitelist)]
    #[storage_mapper("whitelist")]
    fn whitelist(&self) -> VecMapper<u64>;

    #[view(getOg)]
    #[storage_mapper("og")]
    fn og(&self) -> VecMapper<u64>;

    #[view(getPricePublic)]
    #[storage_mapper("pricePublic")]
    fn price_public(&self) -> SingleValueMapper<BigUint>;

    #[view(getPriceWhitelist)]
    #[storage_mapper("priceWhitelist")]
    fn price_whitelist(&self) -> SingleValueMapper<BigUint>;

    #[view(getPriceOg)]
    #[storage_mapper("priceOg")]
    fn price_og(&self) -> SingleValueMapper<BigUint>;

    #[view(getMaximumMintAmountPublic)]
    #[storage_mapper("maximumMintAmountPublic")]
    fn maximum_mint_amount_public(&self) -> SingleValueMapper<u64>;

    #[view(getMaximumMintAmountWhitelist)]
    #[storage_mapper("maximumMintAmountWhitelist")]
    fn maximum_mint_amount_whitelist(&self) -> SingleValueMapper<u64>;

    #[view(getMaximumMintAmountOg)]
    #[storage_mapper("maximumMintAmountOg")]
    fn maximum_mint_amount_og(&self) -> SingleValueMapper<u64>;

    #[view(getPriceTag)]
    #[storage_mapper("priceTag")]
    fn price_tag(&self, nft_nonce: u64) -> SingleValueMapper<PriceTag<Self::Api>>;
}
