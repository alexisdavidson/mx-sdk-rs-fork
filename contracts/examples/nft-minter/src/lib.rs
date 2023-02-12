#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

mod nft_module;

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct ExampleAttributes {
    pub creation_timestamp: u64,
}

#[multiversx_sc::contract]
pub trait NftMinter: nft_module::NftModule {
    #[init]
    fn init(&self) {
        // set default test values
        let name_prefix = sc_format!("Name Prefix");
        self.nft_name_prefix().set(&name_prefix);

        let royalties = 750
        self.royalties().set(&royalties);

        let max_supply = 10
        self.max_supply().set(&max_supply);
        
        let price_public = 10
        self.price_public().set(&price_public);
        
        let price_whitelist = 10
        self.price_whitelist().set(&price_whitelist);
        
        let price_og = 0
        self.price_og().set(&price_og);
        
        let maximum_mint_amount_public = 0
        self.maximum_mint_amount_public().set(&maximum_mint_amount_public);
        
        let maximum_mint_amount_whitelist = 10
        self.maximum_mint_amount_whitelist().set(&maximum_mint_amount_whitelist);
        
        let maximum_mint_amount_og = 1
        self.maximum_mint_amount_og().set(&maximum_mint_amount_og);
        
        let mint_enabled = true
        self.mint_enabled().set(&mint_enabled);
        
        let image_folder_uri = sc_format!("Name Prefix");
        self.image_folder_uri().set(&image_folder_uri);
        
        let image_folder_uri = sc_format!("Name Prefix");
        self.image_folder_uri().set(&image_folder_uri);
        
        let attribute_folder_uri = sc_format!("Name Prefix");
        self.attribute_folder_uri().set(&attribute_folder_uri);
        
        let collection_uri = sc_format!("Name Prefix");
        self.collection_uri().set(&collection_uri);
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::redundant_closure)]
    #[only_owner]
    #[endpoint(createNft)]
    fn create_nft(
        &self,
        name: ManagedBuffer,
        royalties: BigUint,
        uri: ManagedBuffer,
        selling_price: BigUint,
        opt_token_used_as_payment: OptionalValue<TokenIdentifier>,
        opt_token_used_as_payment_nonce: OptionalValue<u64>,
    ) {
        let token_used_as_payment = match opt_token_used_as_payment {
            OptionalValue::Some(token) => EgldOrEsdtTokenIdentifier::esdt(token),
            OptionalValue::None => EgldOrEsdtTokenIdentifier::egld(),
        };
        require!(
            token_used_as_payment.is_valid(),
            "Invalid token_used_as_payment arg, not a valid token ID"
        );

        let token_used_as_payment_nonce = if token_used_as_payment.is_egld() {
            0
        } else {
            match opt_token_used_as_payment_nonce {
                OptionalValue::Some(nonce) => nonce,
                OptionalValue::None => 0,
            }
        };

        let attributes = ExampleAttributes {
            creation_timestamp: self.blockchain().get_block_timestamp(),
        };
        self.create_nft_with_attributes(
            name,
            royalties,
            attributes,
            uri,
            selling_price,
            token_used_as_payment,
            token_used_as_payment_nonce,
        );
    }

    // The marketplace SC will send the funds directly to the initial caller, i.e. the owner
    // The caller has to know which tokens they have to claim,
    // by giving the correct token ID and token nonce
    #[only_owner]
    #[endpoint(claimRoyaltiesFromMarketplace)]
    fn claim_royalties_from_marketplace(
        &self,
        marketplace_address: ManagedAddress,
        token_id: TokenIdentifier,
        token_nonce: u64,
    ) {
        let caller = self.blockchain().get_caller();
        self.marketplace_proxy(marketplace_address)
            .claim_tokens(token_id, token_nonce, caller)
            .async_call()
            .call_and_exit()
    }

    #[proxy]
    fn marketplace_proxy(
        &self,
        sc_address: ManagedAddress,
    ) -> nft_marketplace_proxy::Proxy<Self::Api>;
}

mod nft_marketplace_proxy {
    multiversx_sc::imports!();

    #[multiversx_sc::proxy]
    pub trait NftMarketplace {
        #[endpoint(claimTokens)]
        fn claim_tokens(
            &self,
            token_id: TokenIdentifier,
            token_nonce: u64,
            claim_destination: ManagedAddress,
        );
    }
}
