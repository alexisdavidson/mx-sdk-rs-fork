multiversx_sc::imports!();
multiversx_sc::derive_imports!();

// extern crate alloc;
// use alloc::string::ToString;

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
                self.set_local_roles();
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

    #[endpoint(createAffiliate)]
    fn create_affiliate(&self)-> usize {
        let caller = self.blockchain().get_caller();
        require!(self.get_affiliate_by_address(caller.clone()) == 0, "Already created affiliate");

        self.affiliate_address().push(&caller);

        self.affiliate_address().len()
    }

    fn get_affiliate_by_address(&self, user_address: ManagedAddress)-> usize {
        for i in 1..self.affiliate_address().len() {
            if self.affiliate_address().get(i) == user_address {
                return i
            }
        }
        0
    }

    #[only_owner]
    #[endpoint(airdropNft)]
    fn airdrop_nft(&self, user_address: ManagedAddress, quantity: u64)-> u64 {
        self.perform_mint(user_address, quantity)
    }

    #[payable("*")]
    #[endpoint(mintNft)]
    fn mint_nft(
        &self, 
        quantity: u64,
        opt_affiliate: OptionalValue<usize>
    )-> u64 {
        let (payment_token, payment_amount) = self.call_value().egld_or_single_fungible_esdt();
        let price = self.price_public().get() * quantity; // todo: price depending on whitelist
        require!(payment_amount == price, "The payment must match the mint price");

        
        let affiliate_id = match opt_affiliate {
            OptionalValue::Some(t) => t,
            OptionalValue::None => 0,
        };

        if affiliate_id != 0 {
            require!(affiliate_id <= self.affiliate_address().len(), "Invalid affiliate id");
            let affiliate_address_for_id = self.affiliate_address().get(affiliate_id);
            let percentage_reward = BigUint::from(10u64);
            let affiliate_reward = &price / &percentage_reward;

            // todo: send affiliate reward
            self.send().direct_egld(&affiliate_address_for_id, &affiliate_reward);
        }

        let caller = self.blockchain().get_caller();
        self.perform_mint(caller, quantity)
    }

    fn perform_mint(&self, user_address: ManagedAddress, quantity: u64)-> u64 {
        let name_prefix = self.nft_name_prefix().get();
        let royalties = self.royalties().get();
        let nft_token_id = self.nft_token_id().get();
        let folder_uri = self.image_folder_uri().get();
        let mut current_nft_id = 0u64;
        let nft_tags = self.nft_tags().get();

        for i in 0..quantity {
            current_nft_id = self.amount_minted().get() + 1;

            let name = sc_format!("{} #{}", name_prefix, current_nft_id);
            // QmeWfaLxkCQmK32Lt2ruAeiLvmpbgdVHqpqsB7SKguxfVg/2.png
            let uri = sc_format!("{}/{}.png", folder_uri, current_nft_id);
            let uris = ManagedVec::from_single_item(uri);

            let attribute_uri = self.attribute_folder_uri().get();
            // metadata:QmRturn4WcXAambrzcZqqGcd77HTnvDwYtsCcR1fzfUSgB/2.json;tags:block,slime,rpg
            let attributes = sc_format!("metadata:{}/{}.json;tags:{}", attribute_uri, current_nft_id, nft_tags);
            let mut serialized_attributes = ManagedBuffer::new();
            if let core::result::Result::Err(err) = attributes.top_encode(&mut serialized_attributes) {
                sc_panic!("Attributes encode error: {}", err.message_bytes());
            }
            let attributes_sha256 = self.crypto().sha256(&serialized_attributes);
            let attributes_hash = attributes_sha256.as_managed_buffer();

            let nft_nonce = self.send().esdt_nft_create(
                &nft_token_id,
                &BigUint::from(NFT_AMOUNT),
                &name,
                &royalties,
                attributes_hash,
                &attributes,
                &uris,
            );
            
            self.amount_minted().set(&current_nft_id);

            self.send().direct_esdt(
                &user_address,
                &nft_token_id,
                nft_nonce,
                &BigUint::from(NFT_AMOUNT),
            );
            
        }

        current_nft_id
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
    
    // Set attribute_folder_uri
    #[only_owner]
    #[endpoint]
    fn set_attribute_folder_uri(&self, attribute_folder_uri: ManagedBuffer ) {
        self.attribute_folder_uri().set(&attribute_folder_uri);
    }
    
    // Set nft_name_prefix
    #[only_owner]
    #[endpoint]
    fn set_nft_name_prefix(&self, nft_name_prefix: ManagedBuffer ) {
        self.nft_name_prefix().set(&nft_name_prefix);
    }
    
    // Set nft_tags
    #[only_owner]
    #[endpoint]
    fn set_nft_tags(&self, nft_tags: ManagedBuffer ) {
        self.nft_tags().set(&nft_tags);
    }
    
    // Set collection_uri
    #[only_owner]
    #[endpoint]
    fn set_collection_uri(&self, collection_uri: ManagedBuffer ) {
        self.collection_uri().set(&collection_uri);
    }
    
    // Set max_supply
    #[only_owner]
    #[endpoint]
    fn set_max_supply(&self, max_supply: u64 ) {
        self.max_supply().set(&max_supply);
    }
    
    // Set royalties
    #[only_owner]
    #[endpoint]
    fn set_royalties(&self, royalties: BigUint ) {
        self.royalties().set(&royalties);
    }
    
    // Set price_public
    #[only_owner]
    #[endpoint]
    fn set_price_public(&self, price_public: BigUint ) {
        self.price_public().set(&price_public);
    }
    
    // Set price_whitelist
    #[only_owner]
    #[endpoint]
    fn set_price_whitelist(&self, price_whitelist: BigUint ) {
        self.price_whitelist().set(&price_whitelist);
    }
    
    // Set price_og
    #[only_owner]
    #[endpoint]
    fn set_price_og(&self, price_og: BigUint ) {
        self.price_og().set(&price_og);
    }
    
    // Set maximum_mint_amount_public
    #[only_owner]
    #[endpoint]
    fn set_maximum_mint_amount_public(&self, maximum_mint_amount_public: u64 ) {
        self.maximum_mint_amount_public().set(&maximum_mint_amount_public);
    }
    
    // Set maximum_mint_amount_whitelist
    #[only_owner]
    #[endpoint]
    fn set_maximum_mint_amount_whitelist(&self, maximum_mint_amount_whitelist: u64 ) {
        self.maximum_mint_amount_whitelist().set(&maximum_mint_amount_whitelist);
    }
    
    // Set maximum_mint_amount_og
    #[only_owner]
    #[endpoint]
    fn set_maximum_mint_amount_og(&self, maximum_mint_amount_og: u64 ) {
        self.maximum_mint_amount_og().set(&maximum_mint_amount_og);
    }
    
    // Set amount_minted
    #[only_owner]
    #[endpoint]
    fn set_amount_minted(&self, amount_minted: u64 ) {
        self.amount_minted().set(&amount_minted);
    }
    
    // Set whitelist
    // #[only_owner]
    // #[endpoint]
    // fn set_whitelist(&self, whitelist: u64 ) {
    //     self.whitelist().set(&whitelist);
    // }
    
    // // Set og
    // #[only_owner]
    // #[endpoint]
    // fn set_og(&self, og: u64 ) {
    //     self.og().set(&og);
    // }

    // storage

    #[view(getAmountMinted)]
    #[storage_mapper("amount_minted")]
    fn amount_minted(&self) -> SingleValueMapper<u64>;

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

    #[view(getNftTags)]
    #[storage_mapper("nftTags")]
    fn nft_tags(&self) -> SingleValueMapper<ManagedBuffer>;

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
    fn royalties(&self) -> SingleValueMapper<BigUint>;

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

    #[view(getAffiliateAddress)]
    #[storage_mapper("affiliateAddress")]
    fn affiliate_address(&self) -> VecMapper<ManagedAddress>;

    #[view(getPriceTag)]
    #[storage_mapper("priceTag")]
    fn price_tag(&self, nft_nonce: u64) -> SingleValueMapper<PriceTag<Self::Api>>;
}
