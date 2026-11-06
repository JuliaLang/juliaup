terraform {
  required_version = ">= 1.10"

  backend "s3" {
    bucket       = "julialang-terraform"
    key          = "juliaup.tfstate"
    region       = "us-east-1"
    encrypt      = true
    use_lockfile = true
  }

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }
}

provider "aws" {
  region = "us-east-1"

  # Guard against applying to the wrong account with whatever credentials happen to be active
  allowed_account_ids = ["873569884612"]
}
