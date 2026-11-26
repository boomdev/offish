use ironcalc_base::UserModel;

fn main() -> Result<(), String> {
    let mut book = UserModel::new_empty("Pilot forecast", "en", "UTC", "en")?;
    book.rename_sheet(0, "Assumptions")?;
    book.new_sheet()?;
    book.rename_sheet(1, "Forecast")?;
    book.set_user_input(0, 1, 1, "2500")?;
    book.set_user_input(0, 2, 1, "5")?;
    book.set_user_input(1, 1, 1, "=Assumptions!A1/Assumptions!A2")?;
    book.set_user_input(1, 2, 1, "=1/0")?;
    book.evaluate();
    assert_eq!(book.get_formatted_cell_value(1, 1, 1)?, "500");
    assert_eq!(book.get_formatted_cell_value(1, 2, 1)?, "#DIV/0!");
    book.set_user_input(0, 1, 1, "3200")?;
    book.evaluate();
    assert_eq!(book.get_formatted_cell_value(1, 1, 1)?, "640");
    println!("PASS: cross-sheet references, dependency update, and divide-by-zero");
    Ok(())
}
